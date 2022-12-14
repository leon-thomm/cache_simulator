# A MESI and Dragon Cache Simulator

from os import access
from glob import glob
from pickle import PROTO
from xmlrpc.client import MAXINT
from typing import List, Tuple
from collections import deque


PROTOCOL = 'MESI'
WORD_SIZE = 4       # bytes
ADDRESS_SIZE = 4    # bytes
MEM_LAT = 100       # cpu cycles
CACHE_HIT_LAT = 1   # cpu cycles
BUS_WORD_TF_LAT = 2 # cpu cycles
BLOCK_SIZE = 32     # bytes
CACHE_SIZE = 4096   # bytes
CACHE_ASSOC = 2     # blocks

class Times:
	@staticmethod
	def ask_other_caches():
		# assuming immediate response through wired OR
		return BUS_WORD_TF_LAT * ADDRESS_SIZE//WORD_SIZE
	
	@staticmethod
	def cache_to_cache_transf():
		return BUS_WORD_TF_LAT * BLOCK_SIZE//WORD_SIZE
	
	@staticmethod
	def flush():
		return MEM_LAT
	
	@staticmethod
	def mem_fetch():
		return MEM_LAT
	
	@staticmethod
	def cache_hit():
		return CACHE_HIT_LAT


class Processor:
	def __init__(self, instructions, pid):
		super().__init__()

		self.pid = pid
		self.instructions = instructions
		self.instruction_index = 0
		
		self.cycle_count = 0
		self.compute_cycle_count = 0
		self.load_instruction_count = 0
		self.store_instruction_count = 0
		self.idle_cycles = 0
		
		self.cache = Cache(self)
		self.state = ('Ready',)
	
	def translate_instr(self, instr):
		if type(instr[0]) == int:
			match instr[0]:
				case 0:
					return ('PrRead', instr[1])
				case 1:
					return ('PrWrite', instr[1])
				case 2:
					return ('Other', instr[1])
		else:
			return instr
	
	def prepare(self):
		# update state

		match self.state:
			# 'WaitingForCache' and 'Done' don't have any effect

			case ('ReadyToProceed',):
				self.state = ('Ready',)
			case ('ExecutingOther', n):
				if n == 0:
					self.state = ('Ready',)

		if self.state[0] == 'Ready' and self.instruction_index >= len(self.instructions):
			self.state = ('Done',)
	
	def tick(self, cycles=1):
		# processor is ticked first
		self.cycle_count += cycles

		match self.state:
			case ('ExecutingOther', n):
				self.state = ('ExecutingOther', n-cycles)
			case ('Ready',):
				# debugging output
				# if (self.instruction_index % 10000 == 0):
				# 	print("processor: %d, instruction_index: %d" % (self.pid, self.instruction_index))
    
				# cycles == 1
				if self.instruction_index < len(self.instructions):
					inst = self.translate_instr(self.instructions[self.instruction_index])
					# execute next instruction
					match inst:
						# make sure to call cache *after* updating state, because cache might update proc state
						case ('PrRead', addr):
							self.load_instruction_count += 1
							self.state = ('WaitingForCache',)
							self.cache.pr_sig('PrRead', addr)
						case ('PrWrite', addr):
							self.store_instruction_count += 1
							self.state = ('WaitingForCache',)
							self.cache.pr_sig('PrWrite', addr)
						case ('Other', time):
							self.compute_cycle_count += time
							if time > 0:
								self.state = ('ExecutingOther', time-1)
							else:
								# allow for 0 time instructions
								self.state = ('Ready',)
								self.tick()
					self.instruction_index += 1
				else:
					self.state = ('Done',)
			case ('WaitingForCache',) | ('Done',):
				self.idle_cycles += cycles
				# do nothing
				pass
	
	def proceed(self):
		self.state = ('ReadyToProceed',)

class Cache:
	def __init__(self, proc) -> None:
		super().__init__()

		self.proc = proc
		self.bus = None
		self.data = {
			# addr: [(tag, state), ...]
			# last block is most recently used
			idx: []
			for idx in range((CACHE_SIZE//WORD_SIZE) // CACHE_ASSOC)
		}
		self.state = ('Idle',)
  
		self.cache_misses = 0
		self.cache_hits = 0

		self.private_data_accesses = 0
		self.shared_data_accesses = 0

		# invalidations for MESI, updates for Dragon
		self.num_invalidations_or_updates = 0
	
	def dcache_pos(self, addr):
		"""returns the index and tag of the cache block containing the address"""
		# notice memory is word-addressible, not byte addressible
		num_indices = (CACHE_SIZE//WORD_SIZE) // CACHE_ASSOC
		index = addr % num_indices
		tag = addr // num_indices
		return index, tag


	def state_of(self, addr):
		"""returns the state of the block containing the address"""
		index, tag = self.dcache_pos(addr)
		set = self.data[index]
		for (tag_, state_) in set:
			if tag_ == tag:
				return state_
		return 'I'
	
	def set_state_of(self, addr, state):
		"""sets the state of the block containing the address"""
		index, tag = self.dcache_pos(addr)
		set = self.data[index]
		if state == 'I':
			# remove block
			set.remove((tag, self.state_of(addr)))
		else:
			for i, (tag_, state_) in enumerate(set):
				if tag_ == tag:
					set[i] = (tag_, state)
					break
			else:
				raise Exception('Block not found')
	
	def lru(self, index):
		"""returns the least recently used block in the set"""
		set = self.data[index]
		if len(set) == 0:
			raise Exception('LRU of empty set')
		return set[0]
	
	def access_cached(self, addr):
		"""maintains the LRU policy"""

		index, tag = self.dcache_pos(addr)

		cache_set = self.data[index]
		addr_in_set = any([tag_ == tag for tag_,_ in cache_set])

		if addr_in_set:
			# update MRU
			s = self.state_of(addr)
			cache_set.remove((tag, s))
			cache_set.append((tag, s))
		else:
			raise Exception('Address not cached')
	
	def access_uncached(self, addr, state) -> int:
		"""caches a currently uncached and, if necessary, evicts and returns bus busy time for eviction"""

		index, tag = self.dcache_pos(addr)
		set = self.data[index]
		if any([tag_ == tag and state_ != 'I' for tag_,state_ in set]):
			raise Exception('Address already cached')
		set_is_full = len(set) == CACHE_ASSOC

		t = 0
		if set_is_full:
			lru_state = self.lru(index)[1]
			# evict LRU
			match PROTOCOL:
				case 'MESI':
					match lru_state:
						case 'M':
							t += Times.flush()
				case 'Dragon':
					match lru_state:
						case 'M' | 'Sm':
							t += Times.flush()
			set.pop(0)
		
		set.append((tag, state))
		return t
	
	def prepare(self):
		match self.state:
			case ('ResolvingRequest', 0):
				self.proc.proceed()
				self.state = ('Idle',)
	
	def tick(self, cycles=1):
		# cache is ticked second, after processor
		match self.state:
			case ('ResolvingRequest', t):
				self.state = ('ResolvingRequest', t-cycles)
	
	def pr_sig(self, event, addr):

		s = self.state_of(addr)

		# some helper functions to simplify syntax below

		def access_causes_flush():
			index, tag = self.dcache_pos(addr)
			return s == 'I' and len(self.data[index]) == CACHE_ASSOC

		# shorthands
		def proc_proceed():
			self.proc.proceed()
		
		def bus_send_tx(msg):
			self.bus.send_sig(self, msg)
		
		def transition(new_block_state):
			self.set_state_of(addr, new_block_state)
		
		# state transitions
		def idle():
			self.state = ('Idle',)
		
		def res_req(t):
			self.state = ('ResolvingRequest', t)
		
		def wait_for_bus():
			self.state = ('WaitingForBus', (event, addr))
		
		# aggregators
		def acquire_bus():
			self.bus.acquire(self)
			wait_for_bus()
		
		def hit_imm():
			"""handle an immediate hit, i.e. without bus communication"""
			self.access_cached(addr)
			match Times.cache_hit():
				case 0:
					proc_proceed()
					idle()
				case t:
					res_req(t)
     	
      # list of other caches which have the block cached
		others = [
			c
			for c in self.bus.get_caches(exclude=self)
			if c.state_of(addr) != 'I'
		]

		# state machine
		if PROTOCOL == 'MESI':
			match s:
				case 'I':
					self.cache_misses += 1
					match event:
						case 'PrRead':
							acquire_bus()
						case 'PrWrite':
							if access_causes_flush():
								acquire_bus()
							else:
								bus_send_tx(('BusRdX', addr))
								self.access_uncached(addr, 'M')
								# not a hit, but MESI proceeds immediately
								proc_proceed()
								idle()
				case 'S':
					self.cache_hits += 1
					self.shared_data_accesses += 1
					match event:
						case 'PrRead':
							hit_imm()
						case 'PrWrite':
							bus_send_tx(('BusRdX', addr))
							self.num_invalidations_or_updates += len(others)
							transition('M')
							hit_imm()
				case 'E':
					self.cache_hits += 1
					self.private_data_accesses += 1
					match event:
						case 'PrRead':
							hit_imm()
						case 'PrWrite':
							transition('M')
							hit_imm()
				case 'M':
					self.cache_hits += 1
					self.private_data_accesses += 1
					hit_imm()

		elif PROTOCOL == 'Dragon':
			match s:
				case 'I':
					self.cache_misses += 1
					acquire_bus()
				case 'E':
					self.cache_hits += 1
					self.private_data_accesses += 1
					match event:
						case 'PrRead':
							hit_imm()
						case 'PrWrite':
							transition('M')
							hit_imm()
				case 'Sc':
					self.cache_hits == 1
					self.shared_data_accesses += 1
					match event:
						case 'PrRead':
							hit_imm()
						case 'PrWrite':
							self.num_invalidations_or_updates += len(others)
							acquire_bus()
				case 'Sm':
					self.cache_hits += 1
					self.shared_data_accesses += 1
					match event:
						case 'PrRead':
							hit_imm()
						case 'PrWrite':
							self.num_invalidations_or_updates += len(others)
							acquire_bus()
				case 'M':
					self.cache_hits += 1
					self.private_data_accesses += 1
					hit_imm()


	def pr_sig_bus_ready(self) -> int:
		event, addr = self.state[1]
		s = self.state_of(addr)
		t = 0

		if s != 'I':
			self.access_cached(addr)
		else:
			t += self.access_uncached(addr, 'I')

		# shorthands
		def bus_send_tx(msg):
			self.bus.send_sig(self, msg)

		def transition(new_block_state):
			self.set_state_of(addr, new_block_state)
		
		def error():
			raise Exception('Invalid state')

		# list of other caches which have the block cached
		others = [
			c
			for c in self.bus.get_caches(exclude=self)
			if c.state_of(addr) != 'I'
		]

		others_have_block = len(others) > 0

		# state machine
		if PROTOCOL == 'MESI':
			match s:
				case 'I':
					match event:
						case 'PrRead':
							if others_have_block:
								t += \
									Times.ask_other_caches() + \
									Times.cache_to_cache_transf()
								bus_send_tx(('BusRd', addr))
								transition('S')
							else:
								t += \
									Times.ask_other_caches() + \
									Times.mem_fetch()
								bus_send_tx(('BusRdX', addr))
								transition('E')
						case 'PrWrite':
							# means we had to flush
							# t is already set and address is added above
							bus_send_tx(('BusRdX', addr))
							transition('M')
				case _:
					error()
		
		elif PROTOCOL == 'Dragon':
			match s:
				case 'I':
					match event:
						case 'PrRead':
							if others_have_block:
								t += \
									Times.ask_other_caches() + \
									Times.cache_to_cache_transf()
								bus_send_tx(('BusRd', addr))
								transition('Sc')
							else:
								t += \
									Times.ask_other_caches() + \
									Times.mem_fetch()
								bus_send_tx(('BusRdX', addr))
								transition('E')
						case 'PrWrite':
							if others_have_block:
								t += \
									Times.ask_other_caches() + \
									Times.cache_to_cache_transf()
								bus_send_tx(('BusRd', addr))
								bus_send_tx(('BusUpd', addr))
								transition('Sm')
							else:
								t += \
									Times.ask_other_caches() + \
									Times.mem_fetch()
								bus_send_tx(('BusRdX', addr))
								transition('M')
				case 'E':
					error()
				case 'Sc':
					match event:
						case 'PrRead':
							error()
						case 'PrWrite':
							if others_have_block:
								t += Times.ask_other_caches()
								bus_send_tx(('BusUpd', addr))
								transition('Sm')
							else:
								t += Times.ask_other_caches()
								bus_send_tx(('BusUpd', addr))
								transition('M')
				case 'Sm':
					match event:
						case 'PrRead':
							error()
						case 'PrWrite':
							if others_have_block:
								t += Times.ask_other_caches()
								bus_send_tx(('BusUpd', addr))
							else:
								t += Times.ask_other_caches()
								bus_send_tx(('BusUpd', addr))
								transition('M')
				case 'M':
					error()
		
		t -= 1	# account for current cycle

		self.state = ('ResolvingRequest', t)

		return t
	
	def bus_sig(self, sig) -> int:
		event, addr = sig

		# shorthands
		def transition(new_block_state):
			self.set_state_of(addr, new_block_state)
		
		def error():
			raise Exception('Invalid state')

		
		if PROTOCOL == 'MESI':
			match self.state_of(addr):
				case 'I': pass
				case 'S':
					match event:
						case 'BusRd':   pass
						case 'BusRdX':  transition('I')
				case 'E':
					match event:
						case 'BusRd':   transition('S'); # no flushing here, see assumptions
						case 'BusRdX':  transition('I'); return Times.flush()
				case 'M':
					match event:
						case 'BusRd':   transition('S'); return Times.flush()
						case 'BusRdX':  transition('I'); return Times.flush()
		
		elif PROTOCOL == 'Dragon':
			match self.state_of(addr):
				case 'I': pass
				case 'E':
					match event:
						case 'BusRd':   transition('Sc')
						case 'BusUpd':  error()
				case 'Sc': pass
				case 'Sm':
					match event:
						case 'BusRd':   return Times.flush()
						case 'BusUpd':  transition('Sc')
				case 'M':
					match event:
						case 'BusRd':   transition('Sm')
						case 'BusUpd':  error()
		
		return 0


class Bus:
	def __init__(self, caches):
		super().__init__()
		
		self.caches = caches
		self.state = ('Idle',)
		self.pending_busy_time = 0
		self._cache_requests_queue = deque()
		self._signals_queue = deque()

		self.traffic_bytes = 0
	
	def get_caches(self, exclude=None):
		return [cache for cache in self.caches if cache != exclude]
	
	def prepare(self):
		match self.state:
			case ('Busy', 0):
				self.state = ('Idle',)
		

	def tick(self, cycles):
		# the bus is ticked last, after processor and after cache

		# proceed

		match self.state:
			case ('Busy', t):
				self.state = ('Busy', t-cycles)
			
			case ('Idle',):
				# cycles == 1

				# check if there is pending busy time
				if self.pending_busy_time > 0:
					t = self.pending_busy_time - 1	# account for current cycle
					self.state = ('Busy', t)
					self.pending_busy_time = 0

				# check if there are any bus signals to send
				elif len(self._signals_queue) > 0:
					origin_cache, sig = self._signals_queue.popleft()
					self.pending_busy_time = Times.ask_other_caches()
					for cache in self.get_caches(exclude=origin_cache):
						self.pending_busy_time += cache.bus_sig(sig)
					self.pending_busy_time -= 1  # account for current cycle
					self.state = ('Busy', self.pending_busy_time)
					self.pending_busy_time = 0

				# otherwise, hand over to next cache in queue
				elif len(self._cache_requests_queue) > 0:
					c = self._cache_requests_queue.popleft()
					t = c.pr_sig_bus_ready() + self.pending_busy_time
					# current cycle is already accounted for in pr_sig_bus_ready()
					self.traffic_bytes += BLOCK_SIZE
					self.state = ('Busy', t)
	
	def acquire(self, cache):
		self._cache_requests_queue.append(cache)
	
	def send_sig(self, origin_cache, sig):
		self._signals_queue.append((origin_cache, sig))


def simulate(instructions):
	n = len(instructions)
	procs = [Processor(instructions[i], i) for i in range(n)]
	caches = [procs[i].cache for i in range(n)]
	bus = Bus(caches)
	for c in caches:
		c.bus = bus

	cycle_count = 0
	while(not all([proc.state == ('Done',) for proc in procs])):

		t = 1

		# optimize
		t = min([
			p.state[1] if p.state[0] == 'ExecutingOther' else (
			c.state[1] if c.state[0] == 'ResolvingRequest' else (
			bus.state[1] if bus.state[0] == 'Busy' else 1
			))
			for (p, c) in zip(procs, caches)
		])

		# tick components
		for p in procs:
			p.tick(t)
		for c in caches:
			c.tick(t)
		bus.tick(t)

		cycle_count += t

		# prepare components for next cycle; clean state
		bus.prepare()
		for c in caches:
			c.prepare()
		for p in procs:
			p.prepare()


	# bus might still be busy
	if bus.state[0] == 'Busy':
		cycle_count += bus.state[1]

	# cycle_count -= 1  # last cycle was only last processor jumping from ReadToProceed to Done
	
	print("overall execution cycles: %d" % cycle_count)
	for proc in procs:
		print("cycle count for p%d: %d" % (proc.pid, proc.cycle_count))
	
	for proc in procs:
		print("compute cycles for p%d: %d" % (proc.pid, proc.compute_cycle_count))
	
	for proc in procs:
		print("p%d number of loads: %d number of stores: %d" % (proc.pid, proc.load_instruction_count, proc.store_instruction_count))
	
	for proc in procs:
		print("idle cycles for p%d: %d" % (proc.pid, proc.idle_cycles))
	
	for proc in procs:
		print(proc.cache.cache_misses, proc.cache.cache_misses + proc.cache.cache_hits)
		print("miss rate for p%d: %f" % (proc.pid, float(proc.cache.cache_misses) / float(proc.cache.cache_misses + proc.cache.cache_hits)))
	
	print("bus traffic: %d" % bus.traffic_bytes)
	print("number of invalidations/updates: %d" % sum([proc.cache.num_invalidations_or_updates for proc in procs]))
	
	for proc in procs:
		print("p%d access to private data: %d access to shared data: %d" % (proc.pid, proc.cache.private_data_accesses, proc.cache.shared_data_accesses))
 
	return cycle_count

def read_test_files(testname) -> List[Tuple[int, int]]:
	insts = []
	for fname in sorted(glob(testname+'*.data')):
		with open(fname, 'r') as f:
			insts.append([
				(int(s.split(' ')[0], 10), int(s.split(' ')[1], 16))
				for s in f.readlines()
			])
		
	return insts

if __name__=='__main__':

	TEST_1_PAYLOAD = [
		[
			('PrRead', 0),
			('Other', 3),
			('PrRead', 1),
			('Other', 2),
			('PrWrite', 0),
		],
		[
			('PrRead', 0),
			('Other', 3),
			('PrRead', 1),
			('Other', 2),
			('PrWrite', 0),
		]
	] 	# expected: 352

	TEST_2_PAYLOAD = [ 
		[ 
			('PrWrite', 0), 
			('PrWrite', 512), 
			('PrWrite', 1024), 
		] 
	]	# expected: 104


	print(simulate(read_test_files("../tests/blackscholes")))