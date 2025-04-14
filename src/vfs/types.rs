use std::rc::Rc;

use std::ops::Deref;
use std::borrow::Borrow;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct NameType(Rc<String>);

impl NameType {
    pub fn new(s: String) -> Self {
        NameType(Rc::new(s))
    }
}

impl Deref for NameType {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<str> for NameType {
    fn borrow(&self) -> &str {
        &self
    }
}