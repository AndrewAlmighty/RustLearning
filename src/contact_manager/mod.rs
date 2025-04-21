mod contact;
mod updatable;

use crate::cli::CommandType;
use crate::contact_manager::contact::Contact;
use crate::contact_manager::updatable::{Updatable};


pub struct ContactManager {
    contacts: Vec<Contact>
}

fn update_specific_contact(contact: &mut impl Updatable, address: Option<String>, email: Option<String>, phone: Option<String>, name: Option<String>) -> Result<(), String> {
    address.as_ref().map(|addr| contact.update_address(addr));
    email.as_ref().map(|e| contact.update_email(e));
    phone.as_ref().map(|p| contact.update_phone(p));
    name.as_ref().map(|n| contact.update_name(n));
    Ok(())
}

impl ContactManager {
    pub fn new() -> ContactManager {
        ContactManager { contacts: Vec::new() }
    }

    pub fn handle_command(&mut self, cmd: CommandType) -> Result<(), String> {
        match cmd {
            CommandType::AddContact(name, phone, email, address) => { return self.add_contact(name, phone, email, address); }
            CommandType::UpdateContact(name_or_email, address, email, phone, name) => { return self.update_contact(name_or_email, address, email, phone, name); }
            CommandType::RemoveContact(name_or_email) => { return self.remove_contact(name_or_email); }
            CommandType::SearchContact(name_or_email) => { return self.print_contact(name_or_email);}
            CommandType::Exit => { return Err("Exit command should not reach command manager".to_string()); }
        }
    }

    fn get_all_contacts(&self) -> impl Iterator<Item = &Contact> {
        self.contacts.iter()
    }

    fn get_all_contacts_mutable(&mut self) -> impl Iterator<Item = &mut Contact> {
        self.contacts.iter_mut()
    }

    fn find_contact_by_name(&self, name: &str) -> Option<Contact> {
        let mut c_i = self.get_all_contacts();
        while let Some(contact) = c_i.next() {
            if contact.get_name() == name {
                return Some(contact.clone());
            }
        }
        None
    }

    fn find_contact_by_email(&self, email: &str) -> Option<Contact> {
        let mut c_i = self.get_all_contacts();
        while let Some(contact) = c_i.next() {
            if contact.get_email() == email {
                return Some(contact.clone());
            }
        }
        None
    }

    fn find_contact(&self, name: Option<&str>, email: Option<&str>) -> Option<Contact> {
        if !name.is_none() {
            if let Some(contact) = self.find_contact_by_name(name.unwrap()) {
                return Some(contact.clone());
            }
        }
        if !email.is_none() {
            if let Some(contact) = self.find_contact_by_email(email.unwrap()) {
                return Some(contact.clone());
            }
        }
        None
    }

    fn add_contact(&mut self, name: String, phone: String, email: String, address: String) -> Result<(), String> {
        if self.find_contact(Some(&name), Some(&email)).is_some() {
            return Err("Contact already exists".to_string());
        }

        self.contacts.push(Contact::new(name, phone, email, address));
        Ok(())
    }

    fn print_contact(&self, name_or_email: String) -> Result<(), String> {
        if let Some(contact) = self.find_contact(Some(&name_or_email), Some(&name_or_email)) {
            println!("{}", contact);
            return Ok(())
        }
        else {
            return Err("No such contact".to_string());
        }
    }

    fn remove_contact(&mut self, name_or_email: String) -> Result<(), String> {
        let mut idx = 0usize;
        let mut found = false;
        {
            let mut c_i = self.get_all_contacts();
            while let Some(contact) = c_i.next() {
                if contact.get_name() == name_or_email {
                    found = true;
                    break;
                }

                if contact.get_email() == name_or_email {
                    found = true;
                    break;
                }

                idx += 1;
            }
        }

        if found {
            self.contacts.remove(idx);
            return Ok(());
        }
        else {
            return Err("No such contact".to_string());
        }
    }

    fn update_contact(&mut self, name_or_email: String, address: Option<String>, email: Option<String>, phone: Option<String>, name: Option<String>) -> Result<(), String> {
        if name_or_email.is_empty() {
            return Err("name or email cannot be empty".to_string());
        }
        while let Some(contact) = self.get_all_contacts_mutable().next() {
            if contact.get_name() == name_or_email || contact.get_email() == name_or_email {
                update_specific_contact(contact, address, email, phone, name)?;
                break;
            }
        }

        Ok(())
    }
}