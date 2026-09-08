//! QA category "Tournaments/Events" (1 ruling; excluded, see
//! qa_rulings/mod.rs).
//!
//! QA #7: "Is there a time limit for the game?" -> "When participating
//! in tournaments or events, a time limit is set for each game. Please
//! check the details and floor rules for each specific tournament or
//! event. If you are playing casually ... feel free to take as much
//! time as you like."
//!
//! This is an event-organizer floor rule, not a rule the game state
//! itself enforces -- there is nothing for a 1v1 simulator to assert.
//! No test here; this file exists so the exclusion is visible next to
//! the other four categories rather than a silent gap in the count.
