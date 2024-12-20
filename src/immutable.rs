#[derive(Debug, PartialEq)]
#[repr(transparent)]
pub(crate) struct ImmutableStr(ImmutableStrInner);

#[derive(Debug, PartialEq)]
enum ImmutableStrInner {
    Static(&'static str),
    Owned(String),
}

impl From<&'static str> for ImmutableStr {
    fn from(value: &'static str) -> Self {
        ImmutableStr(ImmutableStrInner::Static(value))
    }
}

impl From<String> for ImmutableStr {
    fn from(value: String) -> Self {
        ImmutableStr(ImmutableStrInner::Owned(value))
    }
}

impl ImmutableStr {
    pub(crate) unsafe fn as_static_str(&self) -> &'static str {
        match self.0 {
            // this is totally safe
            ImmutableStrInner::Static(s) => s,
            // this is wildly unsafe: transmutes &'a str into &'static str
            ImmutableStrInner::Owned(ref s) => std::mem::transmute(s.as_str()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn miri_iso_unsafe_immutablestr_drop() {
        let string = "whatever".to_owned();
        let immutable_str: ImmutableStr = string.into();
        let fake_static_ref = unsafe {
            immutable_str.as_static_str()
        };
        // drop ref
        let _ = fake_static_ref;
        // drop owner
        std::mem::drop(immutable_str);
        assert!(true);
    }
}