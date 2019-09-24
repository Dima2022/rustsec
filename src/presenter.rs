//! Presenter for `rustsec::Report` information.

mod tree;

use self::tree::Tree;
use crate::{
    config::{OutputConfig, OutputFormat},
    prelude::*,
};
use abscissa_core::terminal::{
    self,
    Color::{self, Red, Yellow},
};
use rustsec::{
    cargo_lock::{package, DependencyGraph, Lockfile, Package},
    Vulnerability, Warning,
};
use std::{collections::BTreeSet as Set, io, path::Path};

/// Vulnerability information presenter
#[derive(Clone, Debug)]
pub struct Presenter {
    /// Track packages we've displayed once so we don't show the same dep tree
    // TODO(tarcieri): group advisories about the same package?
    displayed_packages: Set<package::Release>,

    /// Output configuration
    config: OutputConfig,
}

impl Presenter {
    /// Create a new vulnerability information presenter
    pub fn new(config: &OutputConfig) -> Self {
        Self {
            displayed_packages: Set::new(),
            config: config.clone(),
        }
    }

    /// Information to display before a report is generated
    pub fn before_report(&mut self, lockfile_path: &Path, lockfile: &Lockfile) {
        if !self.config.is_quiet() {
            status_ok!(
                "Scanning",
                "{} for vulnerabilities ({} crate dependencies)",
                lockfile_path.display(),
                lockfile.packages.len(),
            );
        }
    }

    /// Print the vulnerability report generated by an audit
    pub fn print_report(&mut self, report: &rustsec::Report, lockfile: &Lockfile) {
        if self.config.format == OutputFormat::Json {
            serde_json::to_writer(io::stdout(), &report).unwrap();
            return;
        }

        if report.vulnerabilities.found {
            status_err!("Vulnerable crates found!");
        } else {
            status_ok!("Success", "No vulnerable packages found");
        }

        let dependency_graph = DependencyGraph::new(lockfile).expect("invalid Cargo.lock file");

        for vulnerability in &report.vulnerabilities.list {
            self.print_vulnerability(vulnerability, &dependency_graph);
        }

        if !report.warnings.is_empty() {
            println!();
            status_warn!("found informational advisories for dependencies");

            for warning in &report.warnings {
                self.print_warning(warning)
            }
        }

        if report.vulnerabilities.found {
            println!();

            if report.vulnerabilities.count == 1 {
                status_err!("1 vulnerability found!");
            } else {
                status_err!("{} vulnerabilities found!", report.vulnerabilities.count);
            }
        }
    }

    /// Print information about the given vulnerability
    fn print_vulnerability(
        &mut self,
        vulnerability: &Vulnerability,
        dependency_graph: &DependencyGraph,
    ) {
        let advisory = &vulnerability.advisory;

        println!();
        self.print_attr(Red, "ID:      ", advisory.id.as_str());
        self.print_attr(Red, "Crate:   ", vulnerability.package.name.as_str());
        self.print_attr(Red, "Version: ", &vulnerability.package.version.to_string());
        self.print_attr(Red, "Date:    ", advisory.date.as_str());

        if let Some(url) = advisory.id.url() {
            self.print_attr(Red, "URL:     ", &url);
        } else if let Some(url) = advisory.url.as_ref() {
            self.print_attr(Red, "URL:     ", url);
        }

        self.print_attr(Red, "Title:   ", &advisory.title);
        self.print_attr(
            Red,
            "Solution: upgrade to",
            &vulnerability
                .versions
                .patched
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .as_slice()
                .join(" OR "),
        );

        self.print_tree(Red, &vulnerability.package, dependency_graph);
    }

    /// Print information about a given warning
    fn print_warning(&mut self, warning: &Warning) {
        println!();

        self.print_attr(Yellow, "Crate:   ", warning.package.as_str());
        self.print_attr(Red, "Message: ", warning.message.as_str());

        if let Some(url) = &warning.url {
            self.print_attr(Yellow, "URL:     ", url);
        }

        // TODO(tarcieri): include full packages in warnings so we can print trees
        // self.print_tree(Yellow, &vulnerability.package, dependency_graph);
    }

    /// Display an attribute of a particular vulnerability
    fn print_attr(&self, color: Color, attr: &str, content: &str) {
        terminal::status::Status::new()
            .bold()
            .color(color)
            .status(attr)
            .print_stdout(content)
            .unwrap();
    }

    /// Print the inverse dependency tree to standard output
    fn print_tree(&mut self, color: Color, package: &Package, dependency_graph: &DependencyGraph) {
        // Only show the tree once per package
        if !self.displayed_packages.insert(package.release()) {
            return;
        }

        if !self.config.show_tree.unwrap_or(true) {
            return;
        }

        terminal::status::Status::new()
            .bold()
            .color(color)
            .status("Dependency tree:")
            .print_stdout("")
            .unwrap();

        let package_node = dependency_graph.nodes()[&package.release()];
        Tree::new(dependency_graph.graph()).print_node(package_node)
    }
}
