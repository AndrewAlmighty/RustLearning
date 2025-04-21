pub(super) trait Updatable {
    fn update_name(&mut self, new_name: &str) -> Result<(), String>;
    fn update_email(&mut self, new_email: &str) -> Result<(), String>;
    fn update_phone(&mut self, new_phone: &str) -> Result<(), String>;
    fn update_address(&mut self, new_address: &str) -> Result<(), String>;
}