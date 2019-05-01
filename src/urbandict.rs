use hyper::client::{Client, connect::Connect};
use serde_json;
use rand::{thread_rng, Rng};
use futures::{Future, Stream};
use url::Url;

#[derive(Deserialize)]
struct Def {
    thumbs_up: u64,
    definition: String
}

#[derive(Deserialize)]
struct Entry {
    list: Vec<Def>
}

const MAX_LEN: usize = 400;
pub fn term<C: Connect + 'static>(client: &Client<C>, term: &str) -> Box<Future<Item=String, Error=String>> {
    let term = term.to_owned();
    let url = Url::parse_with_params("https://api.urbandictionary.com/v0/define", &[("term", &term)]).unwrap();
    box client.get(url.as_str().parse().unwrap())
        .and_then(|res| res.into_body().concat2())
        .map(move |body| {
            let entry: Entry = serde_json::from_slice(&body).unwrap();
            let text = match entry.list.len() {
                0 => None,
                1 => Some(&entry.list[0]),
                _ => {
                    let total_score: u64 = entry.list.iter().map(|e| e.thumbs_up).sum();
                    let mut pick = thread_rng().gen_range(0, total_score);
                    entry.list.iter().filter(|e| {
                        if pick > e.thumbs_up {
                            pick -= e.thumbs_up;
                            false
                        } else {
                            true
                        }
                    }).next()
                }
            };
            match text {
                None => format!("no results for '{}'", term),
                Some(d) => {
                    let text = d.definition.lines().next().unwrap();
                    let cap = MAX_LEN - term.len();
                    let end = text.char_indices()
                        .map(|(i, _)| i)
                        .filter(|&i| i <= cap)
                        .last();

                    if let Some(end) = end {
                        let end = text[.. end].rfind(" ").unwrap_or(end);
                        format!("{}: {} ...", term, &text[.. end])
                    } else {
                        format!("{}: {}", term, text)
                    }
                }
            }
        })
        .map_err(|e| format!("something went wrong: {:?}", e))
}
