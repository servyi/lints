#![feature(register_tool)]
#![register_tool(servyi)]

/// A lease over one child (its constructor documentation is missing —
/// this must be denied).
#[servyi::unsound_constructor]
pub struct Handle<'o, T> {
    ptr: *mut T,
    _b: std::marker::PhantomData<&'o ()>,
}

pub fn make(v: &mut u8) -> Handle<'_, u8> {
    // SAFETY: `v` is a fresh exclusive borrow; the only handle.
    Handle { ptr: v as *mut u8, _b: Default::default() }
}

fn main() {}
