use crate::cron::types::{CronJob, CronStoreData};
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

pub struct CronStore {
    path: PathBuf,
    pub jobs: Vec<CronJob>,
}

impl CronStore {
    pub fn new(data_dir: PathBuf) -> Self {
        let path = data_dir.join("cron.json");
        Self {
            path,
            jobs: Vec::new(),
        }
    }

    pub fn load(&mut self) -> Result<()> {
        if self.path.exists() {
            let content = fs::read_to_string(&self.path)?;
            let data: CronStoreData = serde_json::from_str(&content)?;
            self.jobs = data.jobs;
        } else {
            self.jobs = Vec::new();
        }
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        let data = CronStoreData {
            version: 1,
            jobs: self.jobs.clone(),
        };
        let content = serde_json::to_string_pretty(&data)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, content)?;
        Ok(())
    }

    pub fn add(&mut self, job: CronJob) -> Result<()> {
        self.jobs.push(job);
        self.save()
    }

    pub fn remove(&mut self, id: &str) -> Result<bool> {
        let len_before = self.jobs.len();
        self.jobs.retain(|j| j.id != id);
        let removed = self.jobs.len() < len_before;
        if removed {
            self.save()?;
        }
        Ok(removed)
    }
}
