use std::collections::HashMap;

pub fn process_user(id: &str) -> String {
    format_email(id)
}

fn format_email(s: &str) -> String {
    s.to_lowercase()
}

pub struct UserService {
    pub name: String,
}

impl UserService {
    pub fn greet(&self) -> String {
        process_user(&self.name)
    }
}
