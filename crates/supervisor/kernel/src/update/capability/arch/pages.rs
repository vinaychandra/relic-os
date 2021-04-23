use super::*;

macro_rules! myenabled {
    (f, $($tail:stmt)*) => {};
    (t, $($tail:tt)*) => {$($tail)*};
}

macro_rules! paging_impl {
    ($name: ident, $enable_children: ident) => {
        paste! {
            pub struct [<$name DescriptorWrite>]<'a> {
                page_data: &'a mut Boxed<[ [<$name Entry>] ; 512]>,
                myenabled!($enable_children, children: &'a mut LinkedList<PagingTreeAdapter>),
            }

            pub struct  [<$name DescriptorRead>]<'a> {
                page_data: &'a Boxed<[ [<$name Entry>]; 512]>,
                myenabled!($enable_children, children: &'a LinkedList<PagingTreeAdapter>),
            }
        }
    };
}

trace_macros!(true);
// paging_impl!(PML4, t);

pub struct Test<'a> {
    page_data: &'a mut Boxed<[PML4Entry; 512]>,
    #[cfg(1 == 1)]
    children: &'a mut LinkedList<PagingTreeAdapter>,
}

trace_macros!(false);
