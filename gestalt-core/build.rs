use walkdir::WalkDir;
use regex::Regex;

fn main() {
    // collect all NetMsgs and add them to a lookup table. here be jank.
    // NOTE: does not work with nested modules inside files (`mod inner { some stuff }`)
    // NOTE: works properly with #[cfg(test)], but ONLY if it's annotating a module, e.g.
    // #[cfg(test)]
    // mod test {
    //     ...
    // }
    // and then it only works if there are no non-test messages defined after this block.
    // (The mod block must be at the end of the file)

    let attr_regex = Regex::new(r#"#\[netmsg\([^\)]+\)\]"#).unwrap();
    let struct_regex = Regex::new(r#"(?:pub(?:\(crate\))?)?[[:space:]]+struct[[:space:]]+([A-Za-z0-9]+)[[:space:]]*\{"#).unwrap();
    let test_regex = Regex::new(r#"#\[cfg\(test\)\][[:space:]]+(?:pub[[:space:]]+)?mod[[:space:]]+([A-Za-z0-9_]+)"#).unwrap();

    let mut output = r#"use std::collections::HashMap;
use toolbelt::once::InitOnce;
use crate::net::netmsg::{NetMsg, NetMsgId, NetMsgType};

static NETMSG_LOOKUP_TABLE: InitOnce<HashMap<NetMsgId, NetMsgType>> = InitOnce::uninitialized();

pub(crate) fn get_netmsg_table() -> &'static HashMap<NetMsgId, NetMsgType> {
    //If it's already there, just get it.
    if let Some(out) = NETMSG_LOOKUP_TABLE.try_get() { 
        return out;
    }
    //If not, initialize it.
    NETMSG_LOOKUP_TABLE.get_or_init(|| {
        let mut msgs = HashMap::new();
        "#.to_string();

    for entry in WalkDir::new("src").into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.metadata().is_ok() && e.metadata().unwrap().is_file())
    {
        let contents = std::fs::read_to_string(entry.path()).unwrap();
        for cap in attr_regex.captures_iter(&contents) {
            let mut segments = entry.path().iter().skip(1).map(|os_str| os_str.to_string_lossy().into_owned()).collect::<Vec<_>>();
            if segments.get(segments.len()-1).unwrap() == "mod.rs" {
                segments.remove(segments.len()-1);
            }
            else {
                let last_idx = segments.len()-1;
                let last = segments.get_mut(last_idx).unwrap();
                *last = last[..(last.len()-3)].to_string();
            }
            let before = &contents[..cap.get(0).unwrap().end()];
            let after = &contents[cap.get(0).unwrap().end()..];
            if let Some(cap) = struct_regex.captures_iter(after).next() {
                let mut is_test = false;
                if test_regex.is_match(before) {
                    segments.push("test".to_string());
                    is_test = true;
                }
                segments.push(cap.get(1).unwrap().as_str().to_string());
                output.push_str(&format!("\n{0}        msgs.insert(crate::{1}::net_msg_id(), crate::{1}::net_msg_type());",
                                         if is_test { "        #[cfg(test)] {\n        " } else { "" },
                                         segments.join("::")));
                if is_test {
                    output.push_str("\n        }")
                }
            }
        }
    }
    output.push_str(r#"
        msgs
    }).unwrap()
}
"#);
    std::fs::write("src/net/generated.rs", output).unwrap();
}
