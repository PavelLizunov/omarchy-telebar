//! Local account/hello/settings consumer state. Native lifecycle and side
//! effects remain owned by the driver. Explicit storage only; no live discovery.
use crate::{
    authorization::Runtime,
    commands::{Action, Result},
    engine::Effects,
    local_settings::Settings,
    projection::{number, yes},
    storage::Storage,
    wire::{self, Request},
};
use serde_json::{Value, json};
use std::collections::HashSet;
pub struct Application {
    pub settings: Settings,
    pub catalog: Value,
    pub revision: u64,
    pub windows: HashSet<u64>,
    pub pending_open: Option<(std::time::Instant, Value)>,
    pub notifier: crate::notifications::Notifier,
    pub focus_peer: Option<u64>,
    pub notifications_to_close: Vec<u32>,
    pub quit_deadline: Option<std::time::Instant>,
    pub upload_context: Option<(std::path::PathBuf, Vec<std::path::PathBuf>)>,
    pub clipboard_context: Option<crate::clipboard::Context>,
    pub recording_runtime: Option<std::path::PathBuf>,
    pub catalog_sync_failed: bool,
    pub keyring_environment: Option<std::collections::BTreeMap<String, String>>,
}
impl Application {
    pub fn load(home: &Storage) -> std::io::Result<Self> {
        let raw = home
            .read(".local/share/omagram/accounts.json", 64 * 1024)?
            .and_then(|b| crate::wire::bounded_json(&b, 6, 256, 4096).ok())
            .unwrap_or(Value::Null);
        Ok(Self {
            settings: Settings::load(home)?,
            catalog: crate::accounts::normalize(&raw),
            revision: 0,
            windows: HashSet::new(),
            pending_open: None,
            notifier: crate::notifications::Notifier::default(),
            focus_peer: None,
            notifications_to_close: Vec::new(),
            quit_deadline: None,
            upload_context: None,
            clipboard_context: None,
            recording_runtime: None,
            catalog_sync_failed: false,
            keyring_environment: None,
        })
    }
    pub fn add(
        &mut self,
        home: &Storage,
        runtime: &mut Runtime,
        peer: u64,
        request: Request,
        create: &mut dyn FnMut() -> std::io::Result<i32>,
    ) -> Effects {
        let mut created = None;
        let mut added = None;
        let count = runtime.engine.accounts.len();
        let mut roots = Vec::new();
        let known: HashSet<_> = runtime.engine.accounts.values().map(|a| a.client).collect();
        let mut effects=runtime.engine.request_using(peer,request,std::time::Instant::now(),&mut |command,args,_|{
            if command!="account.add"{return Err("unknown command".into());}
            if count>=10{return Err("maximum of 10 accounts allowed".into());}
            let name=args.get("name");if name.is_some_and(|n|!n.is_null()&&(!n.is_string()||n.as_str().unwrap().chars().count()>80)){return Err("account name must be at most 80 characters".into());}
            let name=match name.and_then(Value::as_str).filter(|s|!s.is_empty()){Some(name)=>name.trim().to_owned(),None=>format!("Account {}",count+1)};
            use std::io::Read;let mut random=[0u8;4];std::fs::File::open("/dev/urandom").and_then(|mut f|f.read_exact(&mut random)).map_err(|_|"the request failed")?;
            let millis=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_|"the request failed")?.as_millis();let entropy=random.iter().map(|b|format!("{b:02x}")).collect::<String>();let id=format!("acc_{millis}_{entropy}");
            if runtime.metadata.contains_key(&id){return Err("the request failed".into());}
            if let (Some((home, _)), Some(recording_runtime)) = (&self.upload_context, &self.recording_runtime) {
                roots = crate::paths::AccountPaths::new(&home.join(".local/share/omagram"), &id)?.media_roots(recording_runtime);
            }
            let client=create().map_err(|_|"Telegram is not available")?;created=Some(client);if client<=0||known.contains(&client){return Err("Telegram is not available".into());}
            let mut catalog=self.catalog.clone();catalog["accounts"].as_array_mut().ok_or("the request failed")?.push(json!({"id":id,"name":name,"phone":"","meId":0}));catalog["active"]=json!(id);
            home.write(".local/share/omagram/accounts.json",catalog.to_string().as_bytes(),0o600).map_err(|_|"the request failed")?;
            self.catalog=catalog;self.revision=self.revision.saturating_add(1);added=Some((id.clone(),name.clone(),client));
            Ok(Action::Reply(json!({"account":{"id":id,"name":name},"activeAccount":id,"accountRevision":self.revision,"auth":{"state":"starting"}})))
        });
        if let Some((id, name, client)) = added {
            let mut registration = runtime
                .register(
                    id.clone(),
                    name.clone(),
                    client,
                    roots,
                    std::time::Instant::now(),
                )
                .expect("unique valid created account/client");
            runtime.engine.active = id.clone();
            effects.native.append(&mut registration.native);
            if let Some((home, databases)) = &mut self.upload_context {
                databases.push(home.join(format!(".local/share/omagram/accounts/{id}/database")));
            }
            if let Some(context) = &mut self.clipboard_context
                && let Some((home, _)) = &self.upload_context
            {
                context
                    .databases
                    .push(home.join(format!(".local/share/omagram/accounts/{id}/database")));
            }
            effects.replies.insert(0,(0,json!({"event":"accountAdded","account":{"id":id,"name":name},"activeAccount":id})));
        } else if let Some(client) = created
            && client > 0
            && !known.contains(&client)
        {
            effects.native.push(crate::engine::NativeRequest {
                client,
                query: json!({"@type":"close"}),
            });
        }
        effects
    }
    /// Persist account metadata only for accepted getMe outcomes. Closed auth
    /// resets live identity, not the persisted catalog's recovery information.
    pub fn sync_accounts(
        &mut self,
        home: &Storage,
        runtime: &Runtime,
        effects: &Effects,
    ) -> std::io::Result<()> {
        let mut catalog = self.catalog.clone();
        for (_, event) in &effects.replies {
            if event["event"] != "me" {
                continue;
            }
            let Some(id) = event["account"].as_str() else {
                continue;
            };
            let Some(meta) = runtime.metadata.get(id) else {
                continue;
            };
            let Some(account) = runtime.engine.accounts.get(id) else {
                continue;
            };
            let Some(row) = catalog["accounts"]
                .as_array_mut()
                .and_then(|rows| rows.iter_mut().find(|r| r["id"] == id))
            else {
                continue;
            };
            row["name"] = json!(meta.name);
            row["phone"] = json!(meta.phone);
            row["meId"] = number(account.state.me_id);
        }
        if catalog != self.catalog {
            if let Err(error) = home.write(
                ".local/share/omagram/accounts.json",
                catalog.to_string().as_bytes(),
                0o600,
            ) {
                self.catalog_sync_failed = true;
                return Err(error);
            }
            self.catalog = catalog;
            self.catalog_sync_failed = false;
        }
        Ok(())
    }
    pub fn owns(command: &str) -> bool {
        matches!(
            command,
            "hello"
                | "ui.open"
                | "ui.focus"
                | "app.quit"
                | "account.list"
                | "account.switch"
                | "account.logout"
                | "settings.set"
                | "settings.playback"
                | "settings.chatListWidth"
                | "settings.reactions"
                | "settings.emoji"
                | "settings.quiet"
                | "settings.sounds"
                | "settings.autoDownload"
                | "sounds.another"
        )
    }
    pub fn retire_peer(&mut self, peer: u64) {
        self.windows.remove(&peer);
        if self.focus_peer == Some(peer) {
            self.focus_peer = None;
            self.notifier.focused = None;
        }
    }
    pub fn accounts(&self, runtime: &Runtime) -> Value {
        json!(runtime.engine.accounts.iter().filter_map(|(id,account)|{
        let meta=runtime.metadata.get(id)?;
        let unread=meta.unread.unwrap_or_else(||account.state.chat_list("main",100).iter().filter(|c|!yes(&c["muted"])).map(|c|crate::projection::integer(&c["unread"],0)).sum());
        Some(json!({"id":id,"name":meta.name,"phone":meta.phone,"meId":number(account.state.me_id),"unread":number(unread),"state":meta.auth["state"]}))
    }).collect::<Vec<_>>())
    }
    pub fn snapshot(&self, runtime: &Runtime, window: bool) -> Result<Value> {
        let id = &runtime.engine.active;
        let account = runtime.engine.accounts.get(id).ok_or("unknown account")?;
        let meta = runtime.metadata.get(id).ok_or("unknown account")?;
        let mut result = json!({"version":"0.1.0","auth":meta.auth,"meId":number(account.state.me_id),"chats":account.state.chat_list("main",100),"open":null,"folders":account.state.folders,"mainPosition":number(account.state.main_position),"settings":self.settings.value,"globalStatus":self.settings.global_status,"calls":account.state.calls.values().collect::<Vec<_>>(),"connection":account.state.connection,"accounts":self.accounts(runtime),"activeAccount":id,"accountRevision":self.revision,"recordedSend":account.state.retained.view(id)});
        if window {
            result["allChats"] = json!(account.state.all_chats(500));
            result["stories"] = json!(account.state.stories.values().collect::<Vec<_>>());
        }
        Ok(result)
    }
    pub fn request(
        &mut self,
        home: &Storage,
        runtime: &mut Runtime,
        peer: u64,
        request: Request,
    ) -> Effects {
        if request.command == "account.switch" {
            return self.switch(home, runtime, peer, request);
        }
        let window =
            self.windows.contains(&peer) || yes(request.args.get("window").unwrap_or(&Value::Null));
        let snapshot = if request.command == "hello" {
            Some(self.snapshot(runtime, window))
        } else {
            None
        };
        let accounts = if request.command == "account.list" {
            Some(json!({"active":runtime.engine.active,"accounts":self.accounts(runtime)}))
        } else {
            None
        };
        let command = request.command.clone();
        let logout = if command == "account.logout" {
            let target = request
                .args
                .get("accountId")
                .filter(|v| wire::truthy(v))
                .or_else(|| request.args.get("id").filter(|v| wire::truthy(v)));
            Some(match target {
                Some(value) => value
                    .as_str()
                    .filter(|id| runtime.engine.accounts.contains_key(*id))
                    .map(str::to_owned)
                    .ok_or_else(|| "unknown account".to_string()),
                None => Ok(if request.account.is_null() {
                    runtime.engine.active.clone()
                } else {
                    request.account.as_str().unwrap_or("").into()
                }),
            })
        } else {
            None
        };
        let account = if request.account.is_null() {
            runtime.engine.active.clone()
        } else {
            request.account.as_str().unwrap_or("").into()
        };
        let mut mark_window = false;
        let mut open = None;
        let mut focus = None;
        let mut quit = false;
        let mut effects = runtime.engine.request_using(
            peer,
            request,
            std::time::Instant::now(),
            &mut |_, args, state| {
                if let Some(logout) = &logout {
                    return logout.clone().map(|account| Action::AccountNative {
                        account,
                        query: json!({"@type":"logOut"}),
                        answer: crate::commands::Answer::Empty,
                    });
                }
                if command == "app.quit" {
                    quit = true;
                    return Ok(Action::Reply(json!({})));
                }
                if command == "ui.focus" {
                    focus = Some(crate::commands::arg_int(args,"chatId",None,None,Some(0))?);
                    return Ok(Action::Reply(json!({})));
                }
                if command == "ui.open" {
                    let target = if args.get("url").is_some() {
                        let url = crate::commands::arg_str(args, "url", 2048, 1)?;
                        if !crate::links::telegram(url) || url.chars().any(|c| c <= ' ') { return Err("only Telegram links open in Telebar".into()); }
                        json!({"url":url,"account":account})
                    } else {
                        json!({"chatId":number(crate::commands::arg_int(args,"chatId",None,None,None)?),"reply":args["reply"]==true,"account":account})
                    };
                    open = Some(target);
                    return Ok(Action::Reply(json!({})));
                }
                if let Some(snapshot) = &snapshot {
                    let mut snapshot = snapshot.clone()?;
                    mark_window = window;
                    if window && let Some((started, target)) = self.pending_open.take() && started.elapsed() < std::time::Duration::from_secs(30) {
                        snapshot["open"] = target;
                    }
                    return Ok(Action::Reply(snapshot));
                }
                if let Some(accounts) = &accounts {
                    return Ok(Action::Reply(accounts.clone()));
                }
                self.settings
                    .command(home, &command, args, state)
                    .unwrap_or_else(|| Err("migration command is not implemented".into()))
            },
        );
        if quit {
            effects.replies.insert(0, (0, json!({"event":"quit"})));
            self.quit_deadline =
                Some(std::time::Instant::now() + std::time::Duration::from_millis(300));
        }
        if mark_window {
            self.windows.insert(peer);
        }
        if let Some(chat) = focus {
            self.focus_peer = (chat != 0).then_some(peer);
            let key = (chat != 0).then(|| crate::notifications::Key {
                account: account.clone(),
                chat,
            });
            if let Some(key) = &key
                && let Some(id) = self.notifier.withdraw(key)
            {
                // Finite identity queue; transport later drains exact withdrawals.
                if self.notifications_to_close.len() >= 512 {
                    self.notifications_to_close.remove(0);
                }
                self.notifications_to_close.push(id);
            }
            self.notifier.focused = key;
            if chat != 0
                && self
                    .settings
                    .value
                    .get("reactionsSeen")
                    .is_none_or(|v| v == true)
                && runtime.engine.accounts[&account]
                    .state
                    .chats
                    .get(&chat)
                    .is_some_and(|c| crate::projection::integer(&c["reactions"], 0) > 0)
            {
                let seen = runtime.read_reactions(&account, chat, std::time::Instant::now());
                effects.native.extend(seen.native);
            }
        }
        if let Some(target) = open {
            if self.windows.is_empty() {
                self.pending_open = Some((std::time::Instant::now(), target));
            } else {
                self.pending_open = None;
                let mut event = target;
                event["event"] = json!("open");
                // Open events precede the admitted terminal response, as in Python.
                let mut replies: Vec<_> = self
                    .windows
                    .iter()
                    .copied()
                    .map(|p| (p, event.clone()))
                    .collect();
                replies.append(&mut effects.replies);
                effects.replies = replies;
            }
        }
        effects.replies.extend(
            std::mem::take(&mut self.settings.events)
                .into_iter()
                .map(|e| (0, e)),
        );
        effects
    }
    pub fn switch(
        &mut self,
        home: &Storage,
        runtime: &mut Runtime,
        peer: u64,
        request: Request,
    ) -> Effects {
        if request.command != "account.switch" {
            return Effects {
                replies: vec![(peer, wire::failure(request.id, "unknown command", 0))],
                ..Effects::default()
            };
        }
        let id = request
            .args
            .get("accountId")
            .filter(|v| wire::truthy(v))
            .or_else(|| request.args.get("id"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        let mut events = Vec::new();
        let result = runtime.engine.request_using(
            peer,
            request,
            std::time::Instant::now(),
            &mut |_, _, _| {
                let id = id
                    .as_deref()
                    .filter(|id| {
                        runtime.metadata.contains_key(*id)
                            && self.catalog["accounts"]
                                .as_array()
                                .is_some_and(|accounts| accounts.iter().any(|a| a["id"] == *id))
                    })
                    .ok_or("unknown account")?;
                let mut catalog = self.catalog.clone();
                catalog["active"] = json!(id);
                home.write(
                    ".local/share/omagram/accounts.json",
                    catalog.to_string().as_bytes(),
                    0o600,
                )
                .map_err(|_| "the request failed")?;
                self.catalog = catalog;
                self.revision = self.revision.saturating_add(1);
                events.push(json!({"event":"accountSwitched","activeAccount":id}));
                Ok(Action::Reply(
                    json!({"activeAccount":id,"accountRevision":self.revision}),
                ))
            },
        );
        // The full selected-account snapshot is assembled after persistence.
        let mut result = result;
        if let Some(id) = id
            && result.replies.iter().any(|(_, r)| r["ok"] == true)
        {
            runtime.engine.active = id;
            let snapshot = self
                .snapshot(runtime, self.windows.contains(&peer))
                .unwrap();
            for (_, reply) in &mut result.replies {
                if reply["ok"] == true {
                    let s = &snapshot;
                    reply["result"] = json!({"activeAccount":s["activeAccount"],"accountRevision":s["accountRevision"],"auth":s["auth"],"meId":s["meId"],"chats":s["chats"],"folders":s["folders"],"mainPosition":s["mainPosition"],"calls":s["calls"],"connection":s["connection"],"recordedSend":s["recordedSend"]});
                    if self.windows.contains(&peer) {
                        reply["result"]["allChats"] = s["allChats"].clone();
                        reply["result"]["stories"] = s["stories"].clone();
                    }
                }
            }
        }
        // Python broadcasts the persisted switch before its terminal reply.
        let mut ordered: Vec<_> = events.into_iter().map(|event| (0, event)).collect();
        ordered.append(&mut result.replies);
        result.replies = ordered;
        result
    }
}
