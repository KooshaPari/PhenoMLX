//! MVP project/epic/story/work-package command surface.

use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
use chrono::NaiveDate;
use clap::{Args, Subcommand, ValueEnum};

use agileplus_domain::{
    domain::{
        cycle::{Cycle, CycleFeature},
        epic::Epic,
        project::Project,
        story::{Story, StoryStatus},
        work_package::{DependencyType, WorkPackage, WpDependency, WpState},
    },
    ports::StoragePort,
};

#[derive(Debug, Subcommand)]
pub enum ProjectCmd {
    /// Create a project.
    Create(ProjectCreateArgs),
}

#[derive(Debug, Args)]
pub struct ProjectCreateArgs {
    #[arg(long)]
    pub slug: String,

    #[arg(long)]
    pub name: String,

    #[arg(long)]
    pub description: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum EpicCmd {
    /// Create an epic.
    Create(EpicCreateArgs),
}

#[derive(Debug, Args)]
pub struct EpicCreateArgs {
    #[arg(long)]
    pub project: i64,

    #[arg(long)]
    pub title: String,

    #[arg(long)]
    pub description: String,

    #[arg(long)]
    pub requirement: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum StoryCmd {
    /// Create a story.
    Create(StoryCreateArgs),
}

#[derive(Debug, Args)]
pub struct StoryCreateArgs {
    #[arg(long)]
    pub epic: i64,

    #[arg(long)]
    pub title: String,

    #[arg(long)]
    pub description: String,

    #[arg(long)]
    pub points: Option<u32>,

    #[arg(long)]
    pub requirement: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum WpCmd {
    /// Create a work package for a story.
    Create(WpCreateArgs),
}

#[derive(Debug, Args)]
pub struct WpCreateArgs {
    #[arg(long)]
    pub story: i64,

    #[arg(long)]
    pub title: String,

    #[arg(long = "file-scope")]
    pub file_scope: String,

    #[arg(long)]
    pub acceptance: String,

    #[arg(long)]
    pub seq: Option<i32>,
}

#[derive(Debug, Subcommand)]
pub enum DepCmd {
    /// Add a work-package dependency.
    Add(DepAddArgs),
}

#[derive(Debug, Args)]
pub struct DepAddArgs {
    #[arg(long)]
    pub wp: i64,

    #[arg(long = "depends-on")]
    pub depends_on: i64,

    #[arg(long = "type", value_enum)]
    pub dep_type: DepTypeArg,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DepTypeArg {
    Explicit,
    FileOverlap,
    Data,
}

#[derive(Debug, Args)]
pub struct CycleCreateArgs {
    #[arg(long)]
    pub name: String,

    #[arg(long)]
    pub start: String,

    #[arg(long)]
    pub end: String,
}

#[derive(Debug, Args)]
pub struct CycleAddArgs {
    #[arg(long)]
    pub cycle: i64,

    #[arg(long, conflicts_with = "story")]
    pub epic: Option<i64>,

    #[arg(long, conflicts_with = "epic")]
    pub story: Option<i64>,
}

#[derive(Debug, Args)]
pub struct TransitionArgs {
    #[arg(long, conflicts_with = "story")]
    pub wp: Option<i64>,

    #[arg(long, conflicts_with = "wp")]
    pub story: Option<i64>,

    #[arg(long)]
    pub to: String,
}

#[derive(Debug, Args)]
pub struct NextReadyArgs {
    #[arg(long)]
    pub cycle: Option<i64>,

    #[arg(long)]
    pub json: bool,
}

pub async fn project_create<S: StoragePort>(args: &ProjectCreateArgs, storage: &S) -> Result<()> {
    let mut project = Project::new(&args.name, &args.slug)?;
    project.description = args.description.clone();
    let id = storage
        .create_project(&project)
        .await
        .context("creating project")?;
    println!("project_id: {id}");
    Ok(())
}

pub async fn epic_create<S: StoragePort>(args: &EpicCreateArgs, storage: &S) -> Result<()> {
    let mut epic = Epic::new(args.project, &args.title)?;
    epic.description = Some(args.description.clone());
    epic.requirement_id = args.requirement.clone();
    let id = storage.create_epic(&epic).await.context("creating epic")?;
    println!("epic_id: {id}");
    Ok(())
}

pub async fn story_create<S: StoragePort>(args: &StoryCreateArgs, storage: &S) -> Result<()> {
    let epic = storage
        .get_epic(args.epic)
        .await
        .context("loading epic")?
        .ok_or_else(|| anyhow!("epic {} not found", args.epic))?;
    let mut story = Story::new(args.epic, epic.project_id, &args.title, args.points)?;
    story.description = Some(args.description.clone());
    story.requirement_id = args.requirement.clone();
    let id = storage
        .create_story(&story)
        .await
        .context("creating story")?;
    println!("story_id: {id}");
    Ok(())
}

pub async fn wp_create<S: StoragePort>(args: &WpCreateArgs, storage: &S) -> Result<()> {
    let seq = args.seq.unwrap_or(1);
    let mut wp = WorkPackage::new(0, &args.title, seq, &args.acceptance);
    wp.file_scope = parse_csv(&args.file_scope);
    let id = storage
        .create_work_package_for_story(args.story, &wp)
        .await
        .context("creating work package")?;
    println!("wp_id: {id}");
    Ok(())
}

pub async fn dep_add<S: StoragePort>(args: &DepAddArgs, storage: &S) -> Result<()> {
    let dep = WpDependency {
        wp_id: args.wp,
        depends_on: args.depends_on,
        dep_type: args.dep_type.into(),
    };
    storage
        .add_wp_dependency(&dep)
        .await
        .context("adding work package dependency")?;
    println!("dependency: {} -> {}", args.wp, args.depends_on);
    Ok(())
}

pub async fn cycle_create<S: StoragePort>(args: &CycleCreateArgs, storage: &S) -> Result<()> {
    let start = parse_date(&args.start)?;
    let end = parse_date(&args.end)?;
    let cycle = Cycle::new(&args.name, start, end, None).map_err(anyhow::Error::msg)?;
    let id = storage
        .create_cycle(&cycle)
        .await
        .context("creating cycle")?;
    println!("cycle_id: {id}");
    Ok(())
}

pub async fn cycle_add<S: StoragePort>(args: &CycleAddArgs, storage: &S) -> Result<()> {
    match (args.epic, args.story) {
        (Some(epic_id), None) => {
            let stories = storage
                .list_stories_by_epic(epic_id)
                .await
                .context("listing epic stories")?;
            if stories.is_empty() {
                bail!("epic {epic_id} has no stories to add");
            }
            for story in stories {
                storage
                    .add_story_to_cycle(args.cycle, story.id)
                    .await
                    .with_context(|| format!("adding story {} to cycle", story.id))?;
            }
            println!("cycle_story_count: added epic {epic_id}");
        }
        (None, Some(story_id)) => {
            storage
                .add_story_to_cycle(args.cycle, story_id)
                .await
                .context("adding story to cycle")?;
            println!("cycle_story: {} -> {}", args.cycle, story_id);
        }
        _ => bail!("provide exactly one of --epic or --story"),
    }
    Ok(())
}

pub async fn cycle_add_feature<S: StoragePort>(
    cycle_id: i64,
    feature_id: i64,
    storage: &S,
) -> Result<()> {
    storage
        .add_feature_to_cycle(&CycleFeature::new(cycle_id, feature_id))
        .await
        .context("adding feature to cycle")?;
    println!("cycle_feature: {cycle_id} -> {feature_id}");
    Ok(())
}

pub async fn transition<S: StoragePort>(args: &TransitionArgs, storage: &S) -> Result<()> {
    match (args.wp, args.story) {
        (Some(wp_id), None) => {
            let target = parse_wp_state(&args.to)?;
            let wp = storage
                .get_work_package(wp_id)
                .await
                .context("loading work package")?
                .ok_or_else(|| anyhow!("work package {wp_id} not found"))?;
            if !wp.state.can_transition_to(target) {
                bail!("illegal wp transition: {:?} -> {:?}", wp.state, target);
            }
            storage
                .update_wp_state(wp_id, target)
                .await
                .context("updating work package state")?;
            println!("wp_state: {wp_id} -> {}", wp_state_label(target));
        }
        (None, Some(story_id)) => {
            let target = StoryStatus::from_str(&args.to)?;
            let mut story = storage
                .get_story(story_id)
                .await
                .context("loading story")?
                .ok_or_else(|| anyhow!("story {story_id} not found"))?;
            story.transition_status(target)?;
            storage
                .update_story_status(story_id, target)
                .await
                .context("updating story status")?;
            println!("story_status: {story_id} -> {target}");
        }
        _ => bail!("provide exactly one of --wp or --story"),
    }
    Ok(())
}

pub async fn next_ready<S: StoragePort>(args: &NextReadyArgs, storage: &S) -> Result<()> {
    let wps = storage
        .get_next_ready_wps(args.cycle)
        .await
        .context("listing next-ready work packages")?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&wps)?);
        return Ok(());
    }

    if wps.is_empty() {
        println!("No next-ready work packages.");
        return Ok(());
    }

    println!("{:<6}  {:<8}  {:<8}  {}", "ID", "FEATURE", "STATE", "TITLE");
    println!("{}", "-".repeat(70));
    for wp in &wps {
        println!(
            "{:<6}  {:<8}  {:<8}  {}",
            wp.id,
            wp.feature_id,
            wp_state_label(wp.state),
            wp.title
        );
    }
    Ok(())
}

fn parse_csv(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_date(raw: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .with_context(|| format!("invalid date '{raw}', expected YYYY-MM-DD"))
}

fn parse_wp_state(raw: &str) -> Result<WpState> {
    match raw {
        "planned" => Ok(WpState::Planned),
        "doing" => Ok(WpState::Doing),
        "review" => Ok(WpState::Review),
        "done" => Ok(WpState::Done),
        "blocked" => Ok(WpState::Blocked),
        _ => bail!("unknown WpState: {raw}"),
    }
}

fn wp_state_label(state: WpState) -> &'static str {
    match state {
        WpState::Planned => "planned",
        WpState::Doing => "doing",
        WpState::Review => "review",
        WpState::Done => "done",
        WpState::Blocked => "blocked",
    }
}

impl From<DepTypeArg> for DependencyType {
    fn from(value: DepTypeArg) -> Self {
        match value {
            DepTypeArg::Explicit => DependencyType::Explicit,
            DepTypeArg::FileOverlap => DependencyType::FileOverlap,
            DepTypeArg::Data => DependencyType::Data,
        }
    }
}
