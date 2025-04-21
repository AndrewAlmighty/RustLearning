use crate::contact_manager::updatable::{Updatable};

use std::fmt;

#[derive(Clone)]
pub(super) struct Contact {
    name: String,
    phone: String,
    email: String,
    address: String
}

impl Contact {
    pub(super) fn new(name: String, phone: String, email: String, address: String) -> Self {
        Contact{name: name, phone: phone, email: email, address: address}
    }

    pub(super) fn get_name(&self) -> &str {
        &self.name
    }

    pub(super) fn get_email(&self) -> &str {
        &self.email
    }
}

impl fmt::Display for Contact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "name: {}, phone: {}, email: {}, address: {}", self.name, self.phone, self.email, self.address)?;
        Ok(())
    }
}

impl Updatable for Contact {
    fn update_name(&mut self, new_name: &str) -> Result<(), String> {
        if new_name.is_empty() {
            return Err("New name cannot be empty".to_string());
        }

        self.name = new_name.to_string();
        Ok(())
    }

    fn update_email(&mut self, new_email: &str) -> Result<(), String> {
        if new_email.is_empty() {
            return Err("New email cannot be empty".to_string());
        }

        self.email = new_email.to_string();
        Ok(())
    }

    fn update_phone(&mut self, new_phone: &str) -> Result<(), String> {
        if new_phone.is_empty() {
            return Err("New phone cannot be empty".to_string());
        }

        self.phone = new_phone.to_string();
        Ok(())
    }

    fn update_address(&mut self, new_address: &str) -> Result<(), String> {
        if new_address.is_empty() {
            return Err("New address cannot be empty".to_string());
        }

        self.address = new_address.to_string();
        Ok(())
    }
}