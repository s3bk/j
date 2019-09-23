#[macro_use] extern crate log;
#[macro_use] extern crate serde_derive;
use std::time::Duration;
use irc::client::prelude::*;
use irc::error::Error as IrcError;
use futures::{future, Future, Stream};
use tokio_core::reactor::Core;
use hyper::Client;
use hyper::client::HttpConnector;
use hyper_rustls::HttpsConnector;
use bullet::eval::EvalContext;
mod urbandict;
mod words;
use words::Words;
mod memo;
use memo::*;
mod util;
use util::*;
use markov::Chain;

pub struct JBot {
    client: Client<HttpsConnector<HttpConnector>>,
    server: IrcServer,
    prefix: String,
    context: EvalContext,
    words: Words,
    memos: Memos,
    chain: Chain<String>
}

fn split(s: &str) -> (&str, Option<&str>) {
    let mut i = s.splitn(2, ' ');
    (i.next().unwrap(), i.next())
}

enum Response {
    Info(&'static str),
    Soon(Box<dyn Future<Item=String, Error=String>>),
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
            "help" => Response::Info("This is J, a bot written in Rust and maintained by sebk (see https://gitlab.com/sebk/j)"),
            "dict" => {
                if let Some(term) = rest {
                    Response::Soon(Box::new(urbandict::term(&self.client, term)))
                } else {
                    Response::Info("usage: dict TERM".into())
                }
            },
            "clear" => {
                self.context = EvalContext::new();
                Response::Empty
            },
            "markov" => {
                Response::Message(self.chain.generate_str())
            }
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
                        Response::Soon(f) => return Box::new(f.or_else(|e| Ok(e)).map(move |msg| irc.send_notice(&to, &msg).unwrap())),
                        Response::Empty => {}
                    }   
                } else if to == self.server.config().nickname() {
                    match self.respond(&from, body) {
                        Response::Error(msg) | Response::Message(msg) => irc.send_privmsg(&from, &msg).unwrap(),
                        Response::Info(msg) => irc.send_privmsg(&from, msg).unwrap(),
                        Response::Soon(f) => return Box::new(f.or_else(|e| Ok(e)).map(move |msg| irc.send_privmsg(&from, &msg).unwrap())),
                        Response::Empty => {}
                    }
                } else {
                    use unicode_segmentation::UnicodeSegmentation;
                    self.words.seen(body.unicode_words());
                    self.chain.feed_str(body);
                }
            },
            Command::PING(ref msg, _) => self.server.send_pong(msg).unwrap(),
            Command::JOIN(ref channel, _, _) => {
                let user = msg.source_nickname().unwrap();
                if let Some(n) = self.memos.has_memos(user, Duration::from_secs(300)) {
                    let msg = format!("Welcome back {}, you have {} memos. type `/msg j memo read` to read.", user, n);
                    irc.send_privmsg(user, &msg).unwrap();
                    irc.send_notice(channel, &msg).unwrap();
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
        
        Box::new(future::ok(()))
    }
    pub fn run(config: &str) {
        let config = Config::load(config).unwrap();
        enum Cause {
            CtrlC,
            Irc(IrcError)
        }
            
        let mut core = Core::new().unwrap();
        loop {
            let irc = IrcServer::new_future(core.handle(), &config).unwrap();
            
            let client = Client::builder()
                .build(HttpsConnector::new(4));

            let ctrl_c = tokio_signal::ctrl_c()
                .map_err(|_| panic!());
            
            let r = core.run(
                irc.map_err(|e| Cause::Irc(e))
                    .join(ctrl_c)
                    .and_then(|(server, ctrl_c)| {
                        let ctrl_c = ctrl_c
                            .map_err(|_| Cause::CtrlC)
                            .then(|_| Err(Cause::CtrlC))
                            .map(|()| panic!());
                        
                        info!("connected");
                        server.identify().unwrap();
                        
                        let mut bot = JBot {
                            prefix: format!("{}:", config.nickname()),
                            client,
                            server: server.clone(),
                            context: EvalContext::new(),
                            words: load("data/words.data"),
                            memos: load("data/memos.data"),
                            chain: Chain::load("data/chain.data").unwrap_or(Chain::new())
                        };

                        server.stream().map_err(|e| Cause::Irc(e))
                            .select(ctrl_c)
                        .for_each(move |msg| bot.handle_msg(msg).map_err(|e| Cause::Irc(e)))
                })
            );
            match r {
                Ok(_) => continue,
                Err(Cause::CtrlC) => break,
                Err(Cause::Irc(_)) => continue
            }
        }
    }
}
impl Drop for JBot {
    fn drop(&mut self) {
        save("data/words.data", &self.words);
        save("data/memos.data", &self.memos);
        self.chain.save("data/chain.dat").unwrap();
    }
}
