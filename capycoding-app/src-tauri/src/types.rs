#[taurpc::ipc_type]
pub struct Info {
    pub id: Id,
    pub first_name: String,
    pub last_name: String,
}

#[taurpc::ipc_type]
pub struct Id {
    _inner: String,
}

impl Id {
    fn new(str: impl Into<String>) -> Self {
        Self { _inner: str.into() }
    }
}
