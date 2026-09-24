//! Titles: a word next to your name that every player sees (Friends, search, profile). You unlock them and
//! equip one. "Owner" is for the admins, "Top 1/2/3" for whoever holds that place on a leaderboard right now,
//! and the rest come from Milestones.

use super::data::{MILESTONES, milestone_cat, milestone_group};
use super::state::GameState;
use crate::gfx::Color;

pub enum Unlock {
    /// accounts in /admins
    Admin,
    /// this place (or better) on any leaderboard tab
    TopRank(i64),
    /// this many milestones claimed (all categories together)
    MilestonesClaimed(usize),
    /// every milestone claimed
    AllMilestones,
    /// every tier of this milestone category
    Category(&'static str),
    /// every tier of every category of this milestone group
    Group(&'static str),
    /// v3.0: this many Prestiges
    Prestige(i64),
}

pub struct TitleDef {
    pub id: &'static str,
    pub name: &'static str,
    pub color: Color,
    pub unlock: Unlock,
}

/// Best first (the picker shows them in this order).
pub const TITLES: [TitleDef; 19] = [
    TitleDef { id: "owner", name: "Owner", color: Color::rgb(255, 84, 84), unlock: Unlock::Admin },
    TitleDef { id: "top1", name: "Top 1", color: Color::rgb(255, 205, 60), unlock: Unlock::TopRank(1) },
    TitleDef { id: "top2", name: "Top 2", color: Color::rgb(205, 215, 230), unlock: Unlock::TopRank(2) },
    TitleDef { id: "top3", name: "Top 3", color: Color::rgb(222, 140, 70), unlock: Unlock::TopRank(3) },
    TitleDef { id: "ascended", name: "Ascended", color: Color::rgb(220, 150, 255), unlock: Unlock::Prestige(5) },
    TitleDef { id: "prestiged", name: "Prestiged", color: Color::rgb(186, 104, 255), unlock: Unlock::Prestige(1) },
    TitleDef { id: "completionist", name: "Completionist", color: Color::rgb(255, 120, 230), unlock: Unlock::AllMilestones },
    TitleDef { id: "master", name: "Master", color: Color::rgb(170, 120, 255), unlock: Unlock::MilestonesClaimed(180) },
    TitleDef { id: "veteran", name: "Veteran", color: Color::rgb(90, 170, 255), unlock: Unlock::MilestonesClaimed(100) },
    TitleDef { id: "collector", name: "Collector", color: Color::rgb(90, 220, 170), unlock: Unlock::MilestonesClaimed(40) },
    TitleDef { id: "rookie", name: "Rookie", color: Color::rgb(170, 200, 140), unlock: Unlock::MilestonesClaimed(10) },
    TitleDef { id: "rarity_hunter", name: "Rarity Hunter", color: Color::rgb(255, 150, 60), unlock: Unlock::Group("rarities") },
    TitleDef { id: "rainbow_chaser", name: "Rainbow Chaser", color: Color::rgb(255, 120, 210), unlock: Unlock::Category("rainbow_rolled") },
    TitleDef { id: "diamond_hands", name: "Diamond Hands", color: Color::rgb(150, 235, 255), unlock: Unlock::Category("diamond_rolled") },
    TitleDef { id: "golden_touch", name: "Golden Touch", color: Color::rgb(255, 205, 60), unlock: Unlock::Category("golden_rolled") },
    TitleDef { id: "trait_master", name: "Trait Master", color: Color::rgb(200, 140, 255), unlock: Unlock::Category("traits_rolled") },
    TitleDef { id: "index_master", name: "Index Master", color: Color::rgb(120, 230, 140), unlock: Unlock::Category("indexed_pets") },
    TitleDef { id: "tycoon", name: "Tycoon", color: Color::rgb(90, 220, 110), unlock: Unlock::Category("coins") },
    TitleDef { id: "roll_addict", name: "Roll Addict", color: Color::rgb(235, 235, 245), unlock: Unlock::Category("rolls") },
];

pub fn title_def(id: &str) -> Option<&'static TitleDef> {
    TITLES.iter().find(|t| t.id == id)
}

/// Canonical &'static id (None for an unknown title, e.g. from a newer version of the game).
pub fn title_key(id: &str) -> Option<&'static str> {
    title_def(id).map(|t| t.id)
}

fn category_done(st: &GameState, category: &str) -> bool {
    match milestone_cat(category) {
        Some(c) => (0..c.tiers.len()).all(|i| st.milestones_claimed.contains(&format!("{}:{}", category, i))),
        None => false,
    }
}

fn total_milestones() -> usize {
    MILESTONES.iter().map(|m| m.tiers.len()).sum()
}

/// Whether this title can be equipped now. `top_rank`: your best place on the leaderboards (None = not listed).
pub fn title_unlocked(t: &TitleDef, st: &GameState, is_admin: bool, top_rank: Option<i64>) -> bool {
    match t.unlock {
        Unlock::Admin => is_admin,
        Unlock::TopRank(n) => top_rank.is_some_and(|r| r <= n),
        Unlock::MilestonesClaimed(n) => st.milestones_claimed.len() >= n,
        Unlock::AllMilestones => st.milestones_claimed.len() >= total_milestones(),
        Unlock::Category(c) => category_done(st, c),
        Unlock::Group(g) => milestone_group(g).is_some_and(|g| g.categories.iter().all(|c| category_done(st, c))),
        Unlock::Prestige(n) => st.prestige >= n,
    }
}

/// How to unlock it (English; the screen translates it).
pub fn title_hint(t: &TitleDef) -> (&'static str, Option<i64>, Option<&'static str>) {
    match t.unlock {
        Unlock::Admin => ("Only for the game's admins.", None, None),
        Unlock::TopRank(1) => ("Be #1 on any leaderboard.", None, None),
        Unlock::TopRank(n) => ("Be in the top %d of any leaderboard.", Some(n), None),
        Unlock::MilestonesClaimed(n) => ("Claim %d milestones.", Some(n as i64), None),
        Unlock::AllMilestones => ("Claim every milestone.", None, None),
        Unlock::Category(c) => ("Complete every \"%s\" milestone.", None, milestone_cat(c).map(|m| m.label)),
        Unlock::Group(g) => ("Complete every \"%s\" milestone.", None, milestone_group(g).map(|m| m.label)),
        Unlock::Prestige(1) => ("Do your first Prestige.", None, None),
        Unlock::Prestige(n) => ("Reach Prestige %d.", Some(n), None),
    }
}
