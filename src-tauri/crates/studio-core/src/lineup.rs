// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};

use crate::models::ManagedChannel;

/// Assign and swap Plex-style guide numbers for the tuner lineup.
pub fn include(channels: &mut [ManagedChannel], include_ids: &[String]) {
    let keep: HashSet<&str> = include_ids
        .iter()
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .collect();
    let mut next = 1;
    for ch in channels.iter() {
        if ch.in_tuner {
            if let Some(n) = ch.tuner_number {
                if n > 0 {
                    next = next.max(n + 1);
                }
            }
        }
    }
    for ch in channels.iter_mut() {
        if !keep.contains(ch.id.as_str()) {
            ch.in_tuner = false;
            ch.tuner_number = None;
            continue;
        }
        ch.in_tuner = true;
        if ch.tuner_number.is_none() || ch.tuner_number.unwrap_or(0) <= 0 {
            ch.tuner_number = Some(next);
            next += 1;
        }
    }
}

pub fn assign_number(channels: &mut [ManagedChannel], channel_id: &str, number: i32) -> Result<(), String> {
    if number < 1 {
        return Err("Channel number must be 1 or greater.".into());
    }
    let idx = channels
        .iter()
        .position(|c| c.id == channel_id)
        .ok_or_else(|| "Channel not found.".to_string())?;
    let previous = channels[idx].tuner_number;
    channels[idx].in_tuner = true;
    let occupant = channels.iter().position(|c| {
        c.in_tuner && c.id != channel_id && c.tuner_number == Some(number)
    });
    channels[idx].tuner_number = Some(number);
    if let Some(oi) = occupant {
        channels[oi].tuner_number = if previous.unwrap_or(0) > 0 {
            previous
        } else {
            Some(next_free(channels, number))
        };
    }
    Ok(())
}

pub fn auto_populate(channels: &mut [ManagedChannel], ids_in_order: &[String]) {
    let mut order = Vec::new();
    let mut seen = HashSet::new();
    for id in ids_in_order {
        if id.trim().is_empty() || !seen.insert(id.as_str()) {
            continue;
        }
        order.push(id.clone());
    }
    let keep: HashSet<&str> = order.iter().map(|s| s.as_str()).collect();
    for ch in channels.iter_mut() {
        if !keep.contains(ch.id.as_str()) {
            ch.in_tuner = false;
            ch.tuner_number = None;
        }
    }
    let mut n = 1;
    for id in &order {
        if let Some(ch) = channels.iter_mut().find(|c| c.id == *id) {
            ch.in_tuner = true;
            ch.tuner_number = Some(n);
            n += 1;
        }
    }
}

pub fn by_number(channels: &[ManagedChannel], number: i32) -> Option<ManagedChannel> {
    ordered_lineup(channels)
        .into_iter()
        .find(|c| c.tuner_number == Some(number))
}

pub fn ordered_lineup(channels: &[ManagedChannel]) -> Vec<ManagedChannel> {
    let mut list: Vec<ManagedChannel> = channels
        .iter()
        .filter(|c| c.in_tuner && c.tuner_number.unwrap_or(0) > 0)
        .cloned()
        .collect();
    list.sort_by(|a, b| {
        a.tuner_number
            .cmp(&b.tuner_number)
            .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
    });
    list
}

/// Playlist order: groups by first appearance (min SortOrder), then channel sort. Not A–Z.
pub fn playlist_order(channels: &[ManagedChannel]) -> Vec<ManagedChannel> {
    let mut group_rank: HashMap<String, i32> = HashMap::new();
    for ch in channels {
        let key = ch.group_title.trim().to_ascii_lowercase();
        group_rank
            .entry(key)
            .and_modify(|min| *min = (*min).min(ch.sort_order))
            .or_insert(ch.sort_order);
    }
    let mut list = channels.to_vec();
    list.sort_by(|a, b| {
        let ra = *group_rank
            .get(&a.group_title.trim().to_ascii_lowercase())
            .unwrap_or(&a.sort_order);
        let rb = *group_rank
            .get(&b.group_title.trim().to_ascii_lowercase())
            .unwrap_or(&b.sort_order);
        ra.cmp(&rb)
            .then(a.sort_order.cmp(&b.sort_order))
            .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
    });
    list
}

pub fn renumber_by_playlist(channels: &mut [ManagedChannel]) {
    let on: Vec<ManagedChannel> = channels.iter().filter(|c| c.in_tuner).cloned().collect();
    let ordered = playlist_order(&on);
    let mut n = 1;
    for o in ordered {
        if let Some(ch) = channels.iter_mut().find(|c| c.id == o.id) {
            ch.tuner_number = Some(n);
            n += 1;
        }
    }
}

fn next_free(channels: &[ManagedChannel], skip: i32) -> i32 {
    let used: HashSet<i32> = channels
        .iter()
        .filter(|c| c.in_tuner)
        .filter_map(|c| c.tuner_number)
        .filter(|n| *n > 0 && *n != skip)
        .collect();
    let mut n = 1;
    while used.contains(&n) {
        n += 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(id: &str, name: &str) -> ManagedChannel {
        ManagedChannel {
            id: id.into(),
            name: name.into(),
            group_title: "Ungrouped".into(),
            tvg_id: None,
            tvg_logo: None,
            notes: None,
            sort_order: 0,
            tvg_shift_hours: 0.0,
            in_tuner: false,
            tuner_number: None,
            variants: vec![],
            has_epg_match: false,
        }
    }

    #[test]
    fn include_assigns_sequential_numbers_starting_at_one() {
        let mut channels = vec![ch("a", "CNN"), ch("b", "ESPN"), ch("c", "BBC")];
        include(&mut channels, &["a".into(), "b".into()]);
        assert!(channels[0].in_tuner);
        assert!(channels[1].in_tuner);
        assert!(!channels[2].in_tuner);
        assert_eq!(channels[0].tuner_number, Some(1));
        assert_eq!(channels[1].tuner_number, Some(2));
        assert_eq!(channels[2].tuner_number, None);
    }

    #[test]
    fn include_keeps_existing_numbers_and_appends_new_at_max_plus_one() {
        let mut channels = vec![ch("a", "CNN"), ch("b", "ESPN"), ch("c", "BBC")];
        channels[0].in_tuner = true;
        channels[0].tuner_number = Some(5);
        include(&mut channels, &["a".into(), "b".into()]);
        assert_eq!(channels[0].tuner_number, Some(5));
        assert_eq!(channels[1].tuner_number, Some(6));
        assert!(!channels[2].in_tuner);
    }

    #[test]
    fn include_drops_unchecked_channels() {
        let mut channels = vec![ch("a", "CNN"), ch("b", "ESPN")];
        channels[0].in_tuner = true;
        channels[0].tuner_number = Some(1);
        channels[1].in_tuner = true;
        channels[1].tuner_number = Some(2);
        include(&mut channels, &["b".into()]);
        assert!(!channels[0].in_tuner);
        assert_eq!(channels[0].tuner_number, None);
        assert!(channels[1].in_tuner);
        assert_eq!(channels[1].tuner_number, Some(2));
    }

    #[test]
    fn assign_number_swaps_when_target_is_already_in_use() {
        let mut channels = vec![ch("espn", "ESPN"), ch("cnn", "CNN")];
        channels[0].in_tuner = true;
        channels[0].tuner_number = Some(12);
        channels[1].in_tuner = true;
        channels[1].tuner_number = Some(5);
        assign_number(&mut channels, "espn", 5).unwrap();
        assert_eq!(channels[0].tuner_number, Some(5));
        assert_eq!(channels[1].tuner_number, Some(12));
    }

    #[test]
    fn assign_number_sets_unused_number_without_touching_others() {
        let mut channels = vec![ch("espn", "ESPN"), ch("cnn", "CNN")];
        channels[0].in_tuner = true;
        channels[0].tuner_number = Some(12);
        channels[1].in_tuner = true;
        channels[1].tuner_number = Some(5);
        assign_number(&mut channels, "espn", 99).unwrap();
        assert_eq!(channels[0].tuner_number, Some(99));
        assert_eq!(channels[1].tuner_number, Some(5));
    }

    #[test]
    fn auto_populate_numbers_from_one_in_given_order() {
        let mut channels = vec![ch("c", "BBC"), ch("a", "CNN"), ch("b", "ESPN")];
        channels[0].in_tuner = true;
        channels[0].tuner_number = Some(99);
        auto_populate(&mut channels, &["a".into(), "b".into()]);
        assert_eq!(channels[1].tuner_number, Some(1));
        assert_eq!(channels[2].tuner_number, Some(2));
        assert!(channels[1].in_tuner);
        assert!(channels[2].in_tuner);
        assert!(!channels[0].in_tuner);
        assert_eq!(channels[0].tuner_number, None);
    }

    #[test]
    fn ordered_lineup_returns_in_tuner_channels_sorted_by_number() {
        let mut channels = vec![ch("b", "ESPN"), ch("skip", "Off"), ch("a", "CNN")];
        channels[0].in_tuner = true;
        channels[0].tuner_number = Some(12);
        channels[2].in_tuner = true;
        channels[2].tuner_number = Some(5);
        let lineup = ordered_lineup(&channels);
        assert_eq!(lineup.len(), 2);
        assert_eq!(lineup[0].name, "CNN");
        assert_eq!(lineup[1].name, "ESPN");
    }

    #[test]
    fn playlist_order_follows_group_appearance_then_channel_sort_not_alpha() {
        let mut channels = vec![
            ch("a", "Zebra"),
            ch("b", "Apple"),
            ch("z", "Alpha"),
            ch("c", "Middle"),
        ];
        channels[0].group_title = "AAA News".into();
        channels[0].sort_order = 50;
        channels[1].group_title = "AAA News".into();
        channels[1].sort_order = 51;
        channels[2].group_title = "24/7".into();
        channels[2].sort_order = 1;
        channels[3].group_title = "24/7".into();
        channels[3].sort_order = 2;
        let ordered = playlist_order(&channels);
        let names: Vec<_> = ordered.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Alpha", "Middle", "Zebra", "Apple"]);
    }

    #[test]
    fn renumber_by_playlist_numbers_in_tuner_rows_in_group_order() {
        let mut channels = vec![ch("a", "Zebra"), ch("z", "Alpha"), ch("off", "Skip")];
        channels[0].group_title = "AAA News".into();
        channels[0].sort_order = 50;
        channels[0].in_tuner = true;
        channels[0].tuner_number = Some(1);
        channels[1].group_title = "24/7".into();
        channels[1].sort_order = 1;
        channels[1].in_tuner = true;
        channels[1].tuner_number = Some(2);
        channels[2].group_title = "24/7".into();
        channels[2].sort_order = 3;
        renumber_by_playlist(&mut channels);
        assert_eq!(channels[1].tuner_number, Some(1));
        assert_eq!(channels[0].tuner_number, Some(2));
        assert_eq!(channels[2].tuner_number, None);
        assert!(!channels[2].in_tuner);
    }
}
