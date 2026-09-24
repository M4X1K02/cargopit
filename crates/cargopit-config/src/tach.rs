//! RevBurner tachometer XML. Points are `SettingsItem` children `Value` (rpm) and `TimeValue` (pulses).

use crate::keys;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TachPoint {
    pub rpm: u32,
    pub pulses: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct XmlElem {
    name: String,
    text: String,
    children: Vec<XmlElem>,
}

pub fn parse_tach_xml(src: &str) -> Result<Vec<TachPoint>, &'static str> {
    let root = parse_elements(src).map_err(|_| "invalid tachometer xml")?;
    let mut points = Vec::new();
    for node in &root {
        collect_settings(node, &mut points);
    }
    if points.is_empty() {
        return Err("tachometer xml contains no settings");
    }
    Ok(points)
}

fn collect_settings(node: &XmlElem, points: &mut Vec<TachPoint>) {
    if node
        .name
        .eq_ignore_ascii_case(keys::TACH_ELEMENT_SETTINGS_ITEM)
    {
        let mut rpm = 0u32;
        let mut pulses = 0u32;
        let mut saw_pulses = false;
        for child in &node.children {
            if child.name.eq_ignore_ascii_case(keys::TACH_ELEMENT_VALUE) {
                rpm = parse_u32(child.text.trim());
            }
            if child
                .name
                .eq_ignore_ascii_case(keys::TACH_ELEMENT_TIME_VALUE)
            {
                pulses = parse_u32(child.text.trim());
                saw_pulses = true;
            }
        }
        if saw_pulses {
            points.push(TachPoint { rpm, pulses });
        }
        return;
    }
    for child in &node.children {
        collect_settings(child, points);
    }
}

fn parse_u32(text: &str) -> u32 {
    let digits: String = text
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == '-' || *ch == '+')
        .collect();
    digits.parse().unwrap_or(0)
}

fn parse_elements(src: &str) -> Result<Vec<XmlElem>, ()> {
    let mut rest = src;
    let mut roots = Vec::new();
    while let Some(next) = skip_prolog(rest) {
        if next.is_empty() {
            break;
        }
        if !next.starts_with('<') {
            return Err(());
        }
        let (elem, after) = parse_element(next)?;
        roots.push(elem);
        rest = after;
    }
    if roots.is_empty() {
        return Err(());
    }
    Ok(roots)
}

fn skip_prolog(src: &str) -> Option<&str> {
    let trimmed = src.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("<?") {
        let end = trimmed.find("?>")?;
        return skip_prolog(&trimmed[end + 2..]);
    }
    if trimmed.starts_with("<!--") {
        let end = trimmed.find("-->")?;
        return skip_prolog(&trimmed[end + 3..]);
    }
    Some(trimmed)
}

fn parse_element(src: &str) -> Result<(XmlElem, &str), ()> {
    if !src.starts_with('<') {
        return Err(());
    }
    let close = src.find('>').ok_or(())?;
    let head = &src[1..close];
    if head.ends_with('/') {
        let name = head
            .trim_end_matches('/')
            .split_whitespace()
            .next()
            .ok_or(())?;
        return Ok((
            XmlElem {
                name: name.to_string(),
                text: String::new(),
                children: Vec::new(),
            },
            &src[close + 1..],
        ));
    }
    let name = head.split_whitespace().next().ok_or(())?;
    let mut rest = &src[close + 1..];
    let mut text = String::new();
    let mut children = Vec::new();
    loop {
        rest = skip_prolog(rest).ok_or(())?;
        if rest.starts_with("</") {
            let end = rest.find('>').ok_or(())?;
            return Ok((
                XmlElem {
                    name: name.to_string(),
                    text,
                    children,
                },
                &rest[end + 1..],
            ));
        }
        if rest.starts_with('<') {
            let (child, after) = parse_element(rest)?;
            children.push(child);
            rest = after;
            continue;
        }
        let next = rest.find('<').ok_or(())?;
        text.push_str(&rest[..next]);
        rest = &rest[next..];
    }
}
