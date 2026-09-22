fn main() {
    let _ = unsafe { [std::mem::transmute::<u32, i32>(7)].iter().count() };
}
