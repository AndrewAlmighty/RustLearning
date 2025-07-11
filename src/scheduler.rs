use crate::cli;

use std::collections::HashMap;
use std::fs::File;
use std::sync::Arc;
use std::io::Write;

use tokio::sync::Mutex;

pub struct Scheduler {
    alerts: Arc<Mutex<HashMap<String, (u32, Option<String>, bool)>>>,
    log_file: Arc<Mutex<File>>
}

impl Scheduler {
    pub fn create() -> Self {
        Scheduler { alerts: Arc::new(Mutex::new(HashMap::new())), log_file: Arc::new(Mutex::new(File::create("alerts_log.txt").expect("Error creating file"))) }
    }

    pub async fn handle_command(&mut self, cmd: cli::CommandType) {
        match cmd {
            cli::CommandType::Create(title, timeout, message) => {               
                let mut alerts = self.alerts.lock().await;
                if alerts.contains_key(&title) {
                    println!("Alert with title {} already exists", title);
                }
                else {
                    alerts.insert(title.clone(), (timeout, message.clone(), true));
                    println!("New alert accepted!");
                    self.spawn_new_timer(title, timeout);
                }
            }
            cli::CommandType::Repeat(title) => {
                let mut alerts = self.alerts.lock().await;
                if let Some(v) = alerts.get_mut(&title) {
                    if v.2 {
                        println!("Alert {} is already set", title);
                    }
                    else {
                        println!("Reseting alert {}", title);
                        v.2 = true;
                        self.spawn_new_timer(title, v.0);
                    }
                }
                else {
                    println!("Cannot repeat alert: {}, not found in db.", title);
                }
            }
            cli::CommandType::Delete(title) => {
                let mut alerts = self.alerts.lock().await;
                if alerts.contains_key(&title) {
                        println!("Deleting alert {}", title);
                        alerts.remove(&title);
                }
                else {
                    println!("Cannot delete alert: {}, not found in db.", title);
                }
            }
        }
    }

    fn spawn_new_timer(&self, title: String, mut timeout: u32) {
        let alerts = Arc::clone(&self.alerts);
        let file = Arc::clone(&self.log_file);

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                let mut alerts = alerts.lock().await;
                if let Some(v) = alerts.get_mut(&title) {
                    if v.2 {
                        if timeout == 0 {
                            v.2 = false;
                            let mut f = file.lock().await;
                            if v.1.is_some() {
                                let _ = f.write_fmt(format_args!("ALARM: {} - {}\n", title, v.1.as_ref().unwrap()));
                            }
                            else {
                                let _ = f.write_fmt(format_args!("ALARM: {}\n", title));
                            }
                        }
                        else { timeout -= 1; }
                    }
                    else { break; }
                }
                else { break; }                            
            }
        });
    }
}