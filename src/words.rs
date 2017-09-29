use chrono::{DateTime, Utc, Datelike};
use std::time::{UNIX_EPOCH, Duration};
use std::collections::hash_map::{HashMap, Entry};

#[derive(Serialize, Deserialize)]
struct WordEntry {
    first_use: u64, // seconds since epoch
    last_use: u64,
    num_uses: usize
}

#[derive(Serialize, Deserialize, Default)]
pub struct Words {
    data: HashMap<String, WordEntry>
}
impl Words {
    pub fn seen<'a, I>(&mut self, words: I) where I: Iterator<Item=&'a str> {
        let now = UNIX_EPOCH.elapsed().unwrap().as_secs();
        for w in words {
            match self.data.entry(w.into()) {
                Entry::Vacant(e) => {
                    e.insert(WordEntry {
                        first_use: now,
                        last_use: now,
                        num_uses: 1
                    });
                },
                Entry::Occupied(mut e) => {
                    let w = e.get_mut();
                    w.num_uses += 1;
                    w.last_use = now;
                }
            }
        }
    }
    pub fn last_seen(&self, word: &str) -> Option<String> {
        self.data.get(word).map(|w| {
            match w.num_uses {
                1 => format!("{} was used once {}", word, date(w.first_use)),
                2 => format!("{} was used twice: {} and {}", word, date(w.first_use), date(w.last_use)),
                n => format!("{} was used {} times between {} and {}",
                             word, n, date(w.first_use), date(w.last_use))
            }
        })
    }
}

fn date(d: u64) -> String {
    let second = 1.0;
    let minute = 60.;
    let hour = 60. * minute;
    let day = 24. * hour;
    let year = 365.25 * day;
    
    let units = [
        ("atoms", second / 376.),
        ("microfortnight", second * 1.2096),
        ("moments", 90. * second),
        ("European swallow-hours per mile", 2.5 * minute),
        ("punct", 15. * minute),
        ("ghurry", 24. * minute),
        ("quadrants", 6. * hour),
        ("nycthemeron", day),
        ("quinzième", 15. * day),
        ("dog years", 52. * day),
        ("seasons", 0.25 * year),
        ("mileways", 5. * year),
    ];

    let now = UNIX_EPOCH.elapsed().unwrap().as_secs();
    let secs = (now - d) as f64;
    for &(name, unit) in units.iter().rev() {
        if unit < secs {
            return format!("{:.1} {} ago", secs / unit, name);
        }
    }

    // fallback
    let t: DateTime<Utc> = (UNIX_EPOCH + Duration::new(d, 0)).into();
    format!("anno {}", t.year())
}
