#[derive(Clone, Debug)]
pub struct Breakpoint {
    pub ty: Type,
    pub addr: u64,
}

#[derive(Clone, Debug)]
pub enum Type {
    R(Len),
    W(Len),
    Rw(Len),
    X,
}

#[derive(Clone, Debug)]
pub enum Len {
    _1,
    _2,
    _3,
    _4,
    _5,
    _6,
    _7,
    _8,
}
