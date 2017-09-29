use std::collections::hash_map::{HashMap};

#[derive(Serialize, Deserialize)]
pub struct Memo {
    pub from: String,
    pub msg: String
}

#[derive(Serialize, Deserialize, Default)]
pub struct Memos {
    memos: HashMap<String, Vec<Memo>>
}
impl Memos {
    pub fn add_memo(&mut self, to: &str, memo: Memo) {
        self.memos.entry(to.to_owned()).or_insert(Vec::new()).push(memo);
    }
    pub fn has_memos(&self, user: &str) -> usize {
        self.memos.get(user).map(|v| v.len()).unwrap_or(0)
    }
    pub fn get_memos(&mut self, user: &str) -> Vec<Memo> {
        self.memos.remove(user).unwrap_or(Vec::new())
    }
}
