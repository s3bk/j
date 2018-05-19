#![feature(box_syntax)]

extern crate irc;
extern crate hyper;
extern crate hyper_tls;
extern crate rand;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_json;
extern crate futures;
extern crate tokio_core;
extern crate tokio_signal;
extern crate url;
#[macro_use] extern crate log;
extern crate bullet;
extern crate bincode;
extern crate chrono;
extern crate unicode_segmentation;

use std::time::Duration;
use irc::client::prelude::*;
use irc::error::Error as IrcError;
use futures::{future, Future, Stream};
use tokio_core::reactor::Core;
use hyper::Client;
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use bullet::eval::EvalContext;
mod urbandict;
mod words;
use words::Words;
mod memo;
use memo::*;
mod util;
use util::*;

pub struct JBot {
    client: Client<HttpsConnector<HttpConnector>>,
    server: IrcServer,
    prefix: String,
    context: EvalContext,
    words: Words,
    memos: Memos
}

fn split(s: &str) -> (&str, Option<&str>) {
    let mut i = s.splitn(2, ' ');
    (i.next().unwrap(), i.next())
}

enum Response {
    Info(&'static str),
    Soon(Box<Future<Item=String, Error=String>>),
    Empty,
    Message(String),
    Error(String)
}


impl JBot {
    fn respond(&mut self, user: &str, msg: &str) -> Response {
        let (first, rest) = split(msg);
        match first {
            "memo" => {
                if let Some(rest) = rest {
                    let (second, rest) = split(rest);
                    match second {
                        "read" => {
                            for memo in self.memos.get_memos(user) {
                                self.server.send_privmsg(user, &format!("{}: {}", memo.from, memo.msg)).unwrap();
                            }
                            Response::Empty
                        },
                        to => {
                            if let Some(msg) = rest {
                                self.memos.add_memo(to, Memo { from: user.into(), msg: msg.into() });
                                Response::Message(format!("added memo for {}", to))
                            } else {
                                Response::Empty
                            }
                        }
                    }
                } else {
                    Response::Info("usage: memo (read | USER message)")
                }
            },
            "word" => {
                if let Some(word) = rest {
                    match self.words.last_seen(word) {
                        Some(last) => Response::Message(last),
                        None => Response::Info("was not seen yet")
                    }
                } else {
                    Response::Info("usage: word TERM")
                }
            },
            "help" => Response::Info("This is J, a bot written in Rust and maintained by sebk (see https://github.com/s3bk/j)"),
            "dict" => {
                if let Some(term) = rest {
                    Response::Soon(box urbandict::term(&self.client, term))
                } else {
                    Response::Info("usage: dict TERM".into())
                }
            },
            "clear" => {
                self.context = EvalContext::new();
                Response::Empty
            },
            _ => match self.context.run(msg) {
                Ok(Some(s)) => Response::Message(s),
                Ok(None) => Response::Empty,
                Err(e) => Response::Error(e.to_string())
            }
        }
    }

    fn handle_msg(&mut self, msg: Message) -> Box<Future<Item=(), Error=irc::error::Error>> {
        debug!("{:?}", msg);
        use irc::proto::command::Command as IrcCommand;
        use irc::proto::response::Response as IrcResponse;
        
        let irc = self.server.clone();
        
        match msg.command {
            Command::PRIVMSG(ref to, ref body) => {
                let from = msg.source_nickname().unwrap().to_owned();
                let to = to.to_owned();
                if body.starts_with(&self.prefix) {
                    let prefix_len = self.prefix.len();
                    match self.respond(&from, body[prefix_len..].trim()) {
                        Response::Error(msg) | Response::Message(msg) => irc.send_notice(&to, &msg).unwrap(),
                        Response::Info(msg) => irc.send_notice(&to, msg).unwrap(),
                        Response::Soon(f) => return box f.or_else(|e| Ok(e)).map(move |msg| irc.send_notice(&to, &msg).unwrap()),
                        Response::Empty => {}
                    }   
                } else if to == self.server.config().nickname() {
                    match self.respond(&from, body) {
                        Response::Error(msg) | Response::Message(msg) => irc.send_privmsg(&from, &msg).unwrap(),
                        Response::Info(msg) => irc.send_privmsg(&from, msg).unwrap(),
                        Response::Soon(f) => return box f.or_else(|e| Ok(e)).map(move |msg| irc.send_privmsg(&from, &msg).unwrap()),
                        Response::Empty => {}
                    }
                } else {
                    use unicode_segmentation::UnicodeSegmentation;
                    self.words.seen(body.unicode_words());
                }
            },
            Command::PING(ref msg, _) => self.server.send_pong(msg).unwrap(),
            Command::JOIN(_, _, _) => {
                let user = msg.source_nickname().unwrap();
                if let Some(n) = self.memos.has_memos(user, Duration::from_secs(300)) {
                    irc.send_privmsg(user, &format!("Welcome back {}, you have {} memos. type `/msg j memo read` to read.", user, n)).unwrap();
                }
            },
            Command::Response(IrcResponse::ERR_NICKNAMEINUSE, _, _) => {
                let config = self.server.config();
                self.server.send(IrcCommand::NICK(format!("{}_", config.nickname()))).unwrap();
                self.server.send_privmsg("NickServ", &format!("RECOVER {} {}", config.nickname(), config.password())).unwrap();
                self.server.identify().unwrap();
            }
            _ => {}
        }
        
        box future::ok(())
    }
    pub fn run(config: &str) {
        let config = Config::load(config).unwrap();
        let mut core = Core::new().unwrap();
        let irc = IrcServer::new_future(core.handle(), &config).unwrap();
        
        let client = Client::configure()
            .connector(HttpsConnector::new(4, &core.handle()).unwrap())
            .build(&core.handle());

        let ctrl_c = tokio_signal::ctrl_c()
            .map_err(|_| panic!());
        
        let r = core.run(
            irc.join(ctrl_c)
                .and_then(|(server, ctrl_c)| {
                    let ctrl_c = ctrl_c
                        .map_err(|_| IrcError::from_kind("CTRL-C".into()))
                        .then(|_| Err(IrcError::from_kind("CTRL-C".into())))
                        .map(|()| panic!());
                    
                    info!("connected");
                    server.identify().unwrap();
                    
                    let mut bot = JBot {
                        prefix: format!("{}:", config.nickname()),
                        client,
                        server: server.clone(),
                        context: EvalContext::new(),
                        words: load("data/words.data"),
                        memos: load("data/memos.data")
                    };

                    server.stream()
                        .select(ctrl_c)
                    .for_each(move |msg| bot.handle_msg(msg))
            })
        );
        println!("{:?}", r);
    }
}
impl Drop for JBot {
    fn drop(&mut self) {
        save("data/words.data", &self.words);
        save("data/memos.data", &self.memos);
    }
}
