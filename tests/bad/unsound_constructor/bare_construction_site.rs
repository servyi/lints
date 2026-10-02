#![feature(register_tool)]
#![register_tool(servyi)]

/// A lease over one child.
///
/// SAFETY (constructor): aliases unless exactly one handle exists per
/// disjoint item; a construction site must prove exactly that.
#[servyi::unsound_constructor]
pub struct Handle<'o, T> {
    ptr: *mut T,
    _b: std::marker::PhantomData<&'o ()>,
}

// The construction site below carries NO SAFETY comment — must be denied.
pub fn make(v: &mut u8) -> Handle<'_, u8> {
    Handle { ptr: v as *mut u8, _b: Default::default() }
}

fn main() {}
