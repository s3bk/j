use std::collections::hash_map::{HashMap};
use std::time::{SystemTime, Duration, UNIX_EPOCH};

#[derive(Serialize, Deserialize)]
pub struct Memo {
    pub from: String,
    pub msg: String
}

#[derive(Serialize, Deserialize)]
pub struct Mailbox {
    entries: Vec<Memo>,
    last_seen: SystemTime,
}
impl Mailbox {
    fn new() -> Mailbox {
        Mailbox {
            entries: Vec::new(),
            last_seen: UNIX_EPOCH
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct Memos {
    memos: HashMap<String, Mailbox>
}
impl Memos {
    pub fn add_memo(&mut self, to: &str, memo: Memo) {
        self.memos.entry(to.to_owned()).or_insert(Mailbox::new()).entries.push(memo);
    }
    pub fn has_memos(&mut self, user: &str, threshold: Duration) -> Option<usize> {
        if let Some(mailbox) = self.memos.get_mut(user) {
            let now = SystemTime::now();
            if now > mailbox.last_seen + threshold {
                mailbox.last_seen = now;
                return Some(mailbox.entries.len());
            }
        }
        None
    }
    pub fn get_memos(&mut self, user: &str) -> Vec<Memo> {
        match self.memos.remove(user) {
            Some(mailbox) => mailbox.entries,
            None => vec![]
        }
    }
}
