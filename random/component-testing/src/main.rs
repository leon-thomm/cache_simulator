struct Item<T>(T);

trait Component {
    type ItemTypes: ComponentItems;
    fn do_something(&mut self, out: &mut Self::ItemTypes);
}

trait ComponentItems {}

macro_rules! impl_component_items {
    ($($T:ident),*) => {
        impl<$($T),*> ComponentItems for ($(Item<$T>,)*) {}
    };
}

// // allowing all kinds of tuples with up to 10 items as 'heterogeneous component item lists'
// impl_component_items!();
// impl_component_items!(T1);
impl_component_items!(T1, T2);
// impl_component_items!(T1, T2, T3);
// impl_component_items!(T1, T2, T3, T4);
// impl_component_items!(T1, T2, T3, T4, T5);
// impl_component_items!(T1, T2, T3, T4, T5, T6);
// impl_component_items!(T1, T2, T3, T4, T5, T6, T7);
// impl_component_items!(T1, T2, T3, T4, T5, T6, T7, T8);
// impl_component_items!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
// impl_component_items!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);

// generates:
//     impl ComponentItems for () {}
//     impl<T> ComponentItems for (Item<T>,) {}
//     impl<T1, T2> ComponentItems for (Item<T1>, Item<T2>) {}
//     impl<T1, T2, T3> ComponentItems for (Item<T1>, Item<T2>, Item<T3>) {}
// ... and so on

// ---- implementation ----

struct MyComponent {}

impl Component for MyComponent {
    type ItemTypes = (Item<u32>, Item<String>);
    fn do_something(&mut self, out: &mut Self::ItemTypes) {
        out.0.0 += 1;   // fine
        // out.1.0 += 1;   // error
        out.1.0 = "hi".into(); // fine
    }
}


fn main() {
    println!("Hello, world!");
}