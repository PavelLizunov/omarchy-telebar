//! Validated profile, proxy and folder mutation preparation.
use crate::{
    commands::{Action, Answer, Result, arg_int, arg_str},
    projection::{number, yes},
    state::State,
};
use serde_json::{Value, json};
pub fn profile(user: &Value, full: &Value, state: &State) -> Value {
    use crate::projection::{integer, text, typed};
    let photo = typed(&user["profile_photo"], "profilePhoto");
    let small = crate::projection::file_view(&photo["small"], &state.roots);
    let id = integer(&photo["id"], 0);
    json!({"userId":number(integer(&user["id"],0)),"firstName":text(&user["first_name"],64),"lastName":text(&user["last_name"],64),"username":text(&user["usernames"]["editable_username"],32),"phone":text(&user["phone_number"],32),"bio":text(&crate::projection::formatted(&full["bio"])[0],140),"photo":if small.is_null(){Value::Null}else{json!({"file":small,"mini":crate::projection::minithumbnail(&photo["minithumbnail"])})},"photoId":if id==0{String::new()}else{id.to_string()}})
}
fn native(query: Value, answer: Answer) -> Action {
    Action::Native { query, answer }
}
pub fn folder(args: &Value) -> Result<Value> {
    let name = arg_str(args, "name", 64, 1)?.trim();
    if !(1..=12).contains(&name.chars().count()) || name.chars().any(|c| c < ' ') {
        return Err("a folder's name is 1-12 characters on one line".into());
    }
    let icons = [
        "All", "Unread", "Unmuted", "Bots", "Channels", "Groups", "Private", "Custom", "Setup",
        "Cat", "Crown", "Favorite", "Flower", "Game", "Home", "Love", "Mask", "Party", "Sport",
        "Study", "Trade", "Travel", "Work", "Airplane", "Book", "Light", "Like", "Money", "Note",
        "Palette",
    ];
    let icon = args["icon"]
        .as_str()
        .filter(|s| icons.contains(s))
        .map(|s| json!({"@type":"chatFolderIcon","name":s}))
        .unwrap_or(Value::Null);
    let color = args["colorId"]
        .as_i64()
        .filter(|n| (-1..=6).contains(n))
        .unwrap_or(-1);
    let mut out = json!({"@type":"chatFolder","name":{"@type":"chatFolderName","animate_custom_emoji":false,"text":{"@type":"formattedText","text":name,"entities":[]}},"icon":icon,"color_id":color,"is_shareable":yes(&args["shareable"])});
    for (key, field) in crate::settings_projection::FOLDER_FLAGS {
        let flag = args
            .get(key)
            .unwrap_or(&Value::Bool(false))
            .as_bool()
            .ok_or_else(|| format!("{key} is true or false"))?;
        out[field] = json!(flag);
    }
    for (key, field) in [
        ("pinned", "pinned_chat_ids"),
        ("included", "included_chat_ids"),
        ("excluded", "excluded_chat_ids"),
    ] {
        let ids = args.get(key).cloned().unwrap_or(json!([]));
        let ids = ids
            .as_array()
            .filter(|a| a.len() <= 200)
            .ok_or_else(|| format!("{key} is a list of at most 200 chat ids"))?;
        let mut clean = Vec::new();
        for id in ids {
            let n = id
                .as_i64()
                .filter(|n| *n != 0 && *n != i64::MIN)
                .ok_or_else(|| format!("{key} is a list of at most 200 chat ids"))?;
            if !clean.contains(&n) {
                clean.push(n);
            }
        }
        out[field] = json!(clean);
    }
    if out["pinned_chat_ids"]
        .as_array()
        .is_none_or(|a| a.is_empty())
        && out["included_chat_ids"]
            .as_array()
            .is_none_or(|a| a.is_empty())
        && !crate::settings_projection::FOLDER_FLAGS
            .iter()
            .any(|(k, f)| k.starts_with("include") && yes(&out[f]))
    {
        return Err("a folder needs chats: a kind of chat it takes, or a chat always in it".into());
    }
    Ok(out)
}
pub fn proxy(args: &Value) -> Result<Value> {
    let server = arg_str(args, "server", 253, 1)?;
    if !server
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || b"-.:".contains(&c))
    {
        return Err("server is a host name or an IP address".into());
    }
    let kind = match args["type"].as_str() {
        Some("mtproto") => {
            let secret = arg_str(args, "secret", 512, 1)?;
            if secret.len() < 16
                || !secret
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_= ".contains(&c) && c != b' ')
            {
                return Err("an MTProto proxy's secret is hexadecimal or base64url".into());
            }
            json!({"@type":"proxyTypeMtproto","secret":secret})
        }
        Some(kind @ ("socks5" | "http")) => {
            let username = if args.get("username").is_some() {
                arg_str(args, "username", 255, 0)?
            } else {
                ""
            };
            let password = if args.get("password").is_some() {
                arg_str(args, "password", 255, 0)?
            } else {
                ""
            };
            if kind == "http" {
                let only = args
                    .get("httpOnly")
                    .map_or(Some(false), Value::as_bool)
                    .ok_or("httpOnly is true or false")?;
                json!({"@type":"proxyTypeHttp","username":username,"password":password,"http_only":only})
            } else {
                json!({"@type":"proxyTypeSocks5","username":username,"password":password})
            }
        }
        _ => return Err("type is socks5, http or mtproto".into()),
    };
    Ok(
        json!({"@type":"proxy","server":server,"port":number(arg_int(args,"port",Some(1),Some(65535),None)?),"type":kind}),
    )
}
pub fn plan(command: &str, args: &Value, state: &State) -> Result<Action> {
    Ok(match command {
        "folder.save" => {
            let folder = folder(args)?;
            if args["id"].is_null() {
                native(
                    json!({"@type":"createChatFolder","folder":folder}),
                    Answer::FolderId,
                )
            } else {
                let id = arg_int(args, "id", Some(1), Some(i32::MAX as i128), None)?;
                native(
                    json!({"@type":"editChatFolder","chat_folder_id":number(id),"folder":folder}),
                    Answer::Constant(json!({"id":number(id)})),
                )
            }
        }
        "folders.reorder" => {
            let ids = args["ids"]
                .as_array()
                .ok_or("ids are every folder's id, in the new order")?;
            let mut actual = Vec::new();
            for id in ids {
                actual.push(
                    id.as_i64()
                        .ok_or("ids are every folder's id, in the new order")?,
                );
            }
            let mut wanted: Vec<_> = state
                .folders
                .iter()
                .filter_map(|f| f["id"].as_i64())
                .collect();
            wanted.sort();
            let mut sorted = actual.clone();
            sorted.sort();
            if sorted != wanted {
                return Err("ids are every folder's id, in the new order".into());
            }
            native(
                json!({"@type":"reorderChatFolders","chat_folder_ids":actual,"main_chat_list_position":number(state.main_position)}),
                Answer::Empty,
            )
        }
        "proxy.add" => native(
            json!({"@type":"addProxy","proxy":proxy(args)?,"enable":true,"comment":""}),
            Answer::Proxy,
        ),
        "password.set" => {
            let old = arg_str(args, "oldPassword", 256, 0)?;
            let new = arg_str(args, "newPassword", 256, 0)?;
            let hint = if new.is_empty() {
                ""
            } else {
                arg_str(args, "hint", 64, 0)?.trim()
            };
            let email = if args["email"].is_null() {
                ""
            } else {
                arg_str(args, "email", 256, 0)?.trim()
            };
            if !hint.is_empty() && hint == new {
                return Err("the hint cannot be the password itself".into());
            }
            if !email.is_empty() && !valid_email(email) {
                return Err("that is not an email address".into());
            }
            native(
                json!({"@type":"setPassword","old_password":old,"new_password":new,"new_hint":hint,"set_recovery_email_address":!email.is_empty(),"new_recovery_email_address":email}),
                Answer::Password,
            )
        }
        _ => return Err("migration command is not implemented".into()),
    })
}
fn valid_email(value: &str) -> bool {
    let Some((name, host)) = value.split_once('@') else {
        return false;
    };
    !name.is_empty()
        && !host.contains('@')
        && !value.contains(' ')
        && host
            .split_once('.')
            .is_some_and(|(a, b)| !a.is_empty() && !b.is_empty())
}
