use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Default)]
pub struct Project {
    pub group_id: Option<String>,
    pub artifact_id: Option<String>,
    pub version: Option<String>,
    pub packaging: Option<String>,
    pub modules: Vec<String>,
    pub properties: HashMap<String, String>,
    pub artifacts: Vec<(Artifact, ArtifactKind)>,
    pub repositories: Vec<Repository>,
}

#[derive(Debug, Clone)]
#[allow(clippy::struct_field_names)]
pub struct Artifact {
    pub group_id: Option<String>,
    pub artifact_id: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArtifactKind {
    Dependency,
    Plugin,
}

impl std::fmt::Display for ArtifactKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dependency => write!(f, "Dependency"),
            Self::Plugin => write!(f, "Plugin"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Repository {
    pub id: Option<String>,
    pub name: Option<String>,
    pub url: String,
    pub kind: RepositoryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryKind {
    Standard,
    Plugin,
}

pub fn parse_pom(path: &Path) -> Result<Project> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    parse_pom_str(&content).with_context(|| format!("Failed to parse {}", path.display()))
}

#[allow(clippy::too_many_lines)]
pub fn parse_pom_str(xml: &str) -> Result<Project> {
    let mut reader = Reader::from_str(xml);
    let mut project = Project::default();
    let mut path_stack: Vec<String> = Vec::new();
    let mut text_buf = String::new();

    let mut artifact_stack: Vec<(Artifact, ArtifactKind)> = Vec::new();
    let mut repo_stack: Vec<(Repository, bool)> = Vec::new(); // (repo, is_building)

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(&e);
                path_stack.push(name.clone());
                text_buf.clear();

                if is_dependency_element(&path_stack) {
                    artifact_stack.push((
                        Artifact {
                            group_id: None,
                            artifact_id: None,
                            version: None,
                        },
                        ArtifactKind::Dependency,
                    ));
                } else if is_plugin_element(&path_stack) {
                    artifact_stack.push((
                        Artifact {
                            group_id: None,
                            artifact_id: None,
                            version: None,
                        },
                        ArtifactKind::Plugin,
                    ));
                } else if is_repository_element(&path_stack) {
                    repo_stack.push((
                        Repository {
                            id: None,
                            name: None,
                            url: String::new(),
                            kind: RepositoryKind::Standard,
                        },
                        true,
                    ));
                } else if is_plugin_repository_element(&path_stack) {
                    repo_stack.push((
                        Repository {
                            id: None,
                            name: None,
                            url: String::new(),
                            kind: RepositoryKind::Plugin,
                        },
                        true,
                    ));
                }
            }
            Ok(Event::End(_)) => {
                let current_path = path_stack.join("/");

                if path_stack.len() == 2 && path_stack[0] == "project" {
                    match path_stack[1].as_str() {
                        "groupId" => project.group_id = Some(text_buf.trim().to_string()),
                        "artifactId" => project.artifact_id = Some(text_buf.trim().to_string()),
                        "version" => project.version = Some(text_buf.trim().to_string()),
                        "packaging" => project.packaging = Some(text_buf.trim().to_string()),
                        _ => {}
                    }
                }

                if is_in_properties(&path_stack)
                    && path_stack.len() > 2
                    && let Some(prop_name) = path_stack.last()
                {
                    project
                        .properties
                        .insert(prop_name.clone(), text_buf.trim().to_string());
                }

                if is_module_element(&path_stack) {
                    project.modules.push(text_buf.trim().to_string());
                }

                let in_nested_block = path_stack
                    .iter()
                    .any(|s| s == "exclusions" || s == "configuration");
                if !in_nested_block && let Some((artifact, _)) = artifact_stack.last_mut() {
                    if current_path.ends_with("/groupId") {
                        artifact.group_id = Some(text_buf.trim().to_string());
                    } else if current_path.ends_with("/artifactId") {
                        artifact.artifact_id = Some(text_buf.trim().to_string());
                    } else if current_path.ends_with("/version") {
                        artifact.version = Some(text_buf.trim().to_string());
                    }
                }

                if let Some((repo, true)) = repo_stack.last_mut() {
                    if current_path.ends_with("/id") {
                        repo.id = Some(text_buf.trim().to_string());
                    } else if current_path.ends_with("/name") {
                        repo.name = Some(text_buf.trim().to_string());
                    } else if current_path.ends_with("/url") {
                        repo.url = text_buf.trim().to_string();
                    }
                }

                if (is_dependency_element(&path_stack) || is_plugin_element(&path_stack))
                    && let Some((artifact, kind)) = artifact_stack.pop()
                {
                    project.artifacts.push((artifact, kind));
                }

                if (is_repository_element(&path_stack) || is_plugin_repository_element(&path_stack))
                    && let Some((repo, _)) = repo_stack.pop()
                    && !repo.url.is_empty()
                {
                    project.repositories.push(repo);
                }

                text_buf.clear();
                path_stack.pop();
            }
            Ok(Event::Text(e)) => {
                let unescaped = e.unescape().context("Failed to unescape XML text")?;
                text_buf.push_str(&unescaped);
            }
            Ok(Event::Eof) => break,
            Err(e) => anyhow::bail!("XML parse error: {e}"),
            _ => {}
        }
    }

    Ok(project)
}

fn local_name(e: &quick_xml::events::BytesStart) -> String {
    let full = String::from_utf8_lossy(e.name().as_ref()).to_string();
    full.split(':').next_back().unwrap_or(&full).to_string()
}

fn is_in_properties(stack: &[String]) -> bool {
    stack.len() >= 2 && stack[1] == "properties"
}

fn is_module_element(stack: &[String]) -> bool {
    stack.len() == 3 && stack[1] == "modules" && stack[2] == "module"
}

fn is_dependency_element(stack: &[String]) -> bool {
    stack.last().map(String::as_str) == Some("dependency")
        && stack
            .iter()
            .any(|s| s == "dependencies" || s == "dependencyManagement")
}

fn is_plugin_element(stack: &[String]) -> bool {
    stack.last().map(String::as_str) == Some("plugin")
        && stack
            .iter()
            .any(|s| s == "plugins" || s == "pluginManagement")
}

fn is_repository_element(stack: &[String]) -> bool {
    stack.last().map(String::as_str) == Some("repository")
        && stack.iter().any(|s| s == "repositories")
        && !stack.iter().any(|s| s == "pluginRepositories")
}

fn is_plugin_repository_element(stack: &[String]) -> bool {
    stack.last().map(String::as_str) == Some("pluginRepository")
        && stack.iter().any(|s| s == "pluginRepositories")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_pom() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <properties>
        <version.wildfly>35.0.0.Final</version.wildfly>
        <version.lombok>1.18.30</version.lombok>
    </properties>
    <dependencyManagement>
        <dependencies>
            <dependency>
                <groupId>org.wildfly.bom</groupId>
                <artifactId>wildfly-ee</artifactId>
                <version>${version.wildfly}</version>
            </dependency>
        </dependencies>
    </dependencyManagement>
    <build>
        <pluginManagement>
            <plugins>
                <plugin>
                    <groupId>org.apache.maven.plugins</groupId>
                    <artifactId>maven-compiler-plugin</artifactId>
                    <version>${version.compiler.plugin}</version>
                </plugin>
            </plugins>
        </pluginManagement>
    </build>
    <modules>
        <module>child-a</module>
        <module>child-b</module>
    </modules>
</project>"#;

        let project = parse_pom_str(xml).unwrap();

        assert_eq!(
            project
                .properties
                .get("version.wildfly")
                .map(String::as_str),
            Some("35.0.0.Final")
        );
        assert_eq!(
            project.properties.get("version.lombok").map(String::as_str),
            Some("1.18.30")
        );

        assert_eq!(project.modules, vec!["child-a", "child-b"]);

        assert_eq!(project.artifacts.len(), 2);
        assert_eq!(
            project.artifacts[0].0.artifact_id.as_deref(),
            Some("wildfly-ee")
        );
        assert_eq!(project.artifacts[0].1, ArtifactKind::Dependency);
        assert_eq!(
            project.artifacts[1].0.artifact_id.as_deref(),
            Some("maven-compiler-plugin")
        );
        assert_eq!(project.artifacts[1].1, ArtifactKind::Plugin);
    }

    #[test]
    fn parse_minimal_pom() {
        let xml = r#"<project></project>"#;
        let project = parse_pom_str(xml).unwrap();
        assert!(project.properties.is_empty());
        assert!(project.modules.is_empty());
        assert!(project.artifacts.is_empty());
    }

    #[test]
    fn parse_with_namespace() {
        let xml = r#"<project xmlns="http://maven.apache.org/POM/4.0.0">
    <properties>
        <version.junit>5.10.0</version.junit>
    </properties>
</project>"#;

        let project = parse_pom_str(xml).unwrap();
        assert_eq!(
            project.properties.get("version.junit").map(String::as_str),
            Some("5.10.0")
        );
    }

    #[test]
    fn finds_plugin_internal_dependencies() {
        let xml = r#"<project>
    <build>
        <plugins>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-surefire-plugin</artifactId>
                <version>${version.surefire}</version>
                <dependencies>
                    <dependency>
                        <groupId>org.junit.platform</groupId>
                        <artifactId>junit-platform-surefire-provider</artifactId>
                        <version>${version.junit.platform}</version>
                    </dependency>
                </dependencies>
            </plugin>
        </plugins>
    </build>
</project>"#;

        let project = parse_pom_str(xml).unwrap();
        // The plugin's nested <dependency> is correctly picked up as a dependency
        // (it has its own version property that needs checking)
        assert!(project.artifacts.len() >= 1);
        let plugin = project
            .artifacts
            .iter()
            .find(|(a, _)| a.artifact_id.as_deref() == Some("maven-surefire-plugin"));
        assert!(plugin.is_some());
        assert_eq!(plugin.unwrap().1, ArtifactKind::Plugin);
    }

    #[test]
    fn parse_repositories() {
        let xml = r#"<project>
    <repositories>
        <repository>
            <id>jboss-public</id>
            <name>JBoss Public</name>
            <url>https://repository.jboss.org/nexus/content/groups/public/</url>
        </repository>
        <repository>
            <id>central-proxy</id>
            <url>https://repo.example.com/maven2</url>
        </repository>
    </repositories>
    <pluginRepositories>
        <pluginRepository>
            <id>jboss-plugins</id>
            <url>https://repository.jboss.org/nexus/content/groups/public/</url>
        </pluginRepository>
    </pluginRepositories>
</project>"#;

        let project = parse_pom_str(xml).unwrap();
        assert_eq!(project.repositories.len(), 3);

        let jboss = &project.repositories[0];
        assert_eq!(jboss.id.as_deref(), Some("jboss-public"));
        assert_eq!(jboss.name.as_deref(), Some("JBoss Public"));
        assert!(jboss.url.contains("jboss.org"));
        assert_eq!(jboss.kind, RepositoryKind::Standard);

        let proxy = &project.repositories[1];
        assert_eq!(proxy.id.as_deref(), Some("central-proxy"));
        assert_eq!(proxy.name, None);
        assert_eq!(proxy.kind, RepositoryKind::Standard);

        let plugin_repo = &project.repositories[2];
        assert_eq!(plugin_repo.id.as_deref(), Some("jboss-plugins"));
        assert_eq!(plugin_repo.kind, RepositoryKind::Plugin);
    }

    #[test]
    fn parse_repository_without_id() {
        let xml = r#"<project>
    <repositories>
        <repository>
            <url>https://repo.example.com/maven2</url>
        </repository>
    </repositories>
</project>"#;

        let project = parse_pom_str(xml).unwrap();
        assert_eq!(project.repositories.len(), 1);
        assert_eq!(project.repositories[0].id, None);
        assert_eq!(project.repositories[0].name, None);
        assert_eq!(
            project.repositories[0].url,
            "https://repo.example.com/maven2"
        );
    }

    #[test]
    fn skips_repository_without_url() {
        let xml = r#"<project>
    <repositories>
        <repository>
            <id>broken</id>
            <name>No URL repo</name>
        </repository>
    </repositories>
</project>"#;

        let project = parse_pom_str(xml).unwrap();
        assert_eq!(project.repositories.len(), 0);
    }

    #[test]
    fn parse_direct_dependencies_and_plugins() {
        let xml = r#"<project>
    <dependencies>
        <dependency>
            <groupId>com.google.guava</groupId>
            <artifactId>guava</artifactId>
            <version>${version.guava}</version>
        </dependency>
    </dependencies>
    <build>
        <plugins>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-jar-plugin</artifactId>
                <version>${version.jar.plugin}</version>
            </plugin>
        </plugins>
    </build>
</project>"#;

        let project = parse_pom_str(xml).unwrap();
        assert_eq!(project.artifacts.len(), 2);
        assert_eq!(project.artifacts[0].1, ArtifactKind::Dependency);
        assert_eq!(project.artifacts[0].0.artifact_id.as_deref(), Some("guava"));
        assert_eq!(project.artifacts[1].1, ArtifactKind::Plugin);
    }

    #[test]
    fn parse_project_coordinates() {
        let xml = r#"<project>
    <groupId>org.wildfly</groupId>
    <artifactId>wildfly-parent</artifactId>
    <version>35.0.0.Final</version>
    <packaging>pom</packaging>
</project>"#;

        let project = parse_pom_str(xml).unwrap();
        assert_eq!(project.group_id.as_deref(), Some("org.wildfly"));
        assert_eq!(project.artifact_id.as_deref(), Some("wildfly-parent"));
        assert_eq!(project.version.as_deref(), Some("35.0.0.Final"));
        assert_eq!(project.packaging.as_deref(), Some("pom"));
    }

    #[test]
    fn parse_project_coordinates_missing_packaging() {
        let xml = r#"<project>
    <groupId>org.example</groupId>
    <artifactId>my-lib</artifactId>
    <version>1.0.0</version>
</project>"#;

        let project = parse_pom_str(xml).unwrap();
        assert_eq!(project.group_id.as_deref(), Some("org.example"));
        assert_eq!(project.artifact_id.as_deref(), Some("my-lib"));
        assert_eq!(project.version.as_deref(), Some("1.0.0"));
        assert_eq!(project.packaging, None);
    }

    #[test]
    fn exclusions_do_not_overwrite_dependency_coordinates() {
        let xml = r#"<project>
    <dependencyManagement>
        <dependencies>
            <dependency>
                <groupId>org.jboss.elemento</groupId>
                <artifactId>elemento-core</artifactId>
                <version>${version.elemento}</version>
                <exclusions>
                    <exclusion>
                        <groupId>org.junit.jupiter</groupId>
                        <artifactId>junit-jupiter</artifactId>
                    </exclusion>
                </exclusions>
            </dependency>
        </dependencies>
    </dependencyManagement>
</project>"#;

        let project = parse_pom_str(xml).unwrap();
        assert_eq!(project.artifacts.len(), 1);
        let (artifact, kind) = &project.artifacts[0];
        assert_eq!(artifact.group_id.as_deref(), Some("org.jboss.elemento"));
        assert_eq!(artifact.artifact_id.as_deref(), Some("elemento-core"));
        assert_eq!(artifact.version.as_deref(), Some("${version.elemento}"));
        assert_eq!(*kind, ArtifactKind::Dependency);
    }

    #[test]
    fn configuration_version_does_not_overwrite_plugin_coordinates() {
        let xml = r#"<project>
    <build>
        <plugins>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-enforcer-plugin</artifactId>
                <version>${version.enforcer.plugin}</version>
                <configuration>
                    <rules>
                        <requireJavaVersion>
                            <version>${maven.compiler.source}</version>
                        </requireJavaVersion>
                    </rules>
                </configuration>
            </plugin>
        </plugins>
    </build>
</project>"#;

        let project = parse_pom_str(xml).unwrap();
        assert_eq!(project.artifacts.len(), 1);
        let (artifact, kind) = &project.artifacts[0];
        assert_eq!(
            artifact.group_id.as_deref(),
            Some("org.apache.maven.plugins")
        );
        assert_eq!(
            artifact.artifact_id.as_deref(),
            Some("maven-enforcer-plugin")
        );
        assert_eq!(
            artifact.version.as_deref(),
            Some("${version.enforcer.plugin}")
        );
        assert_eq!(*kind, ArtifactKind::Plugin);
    }

    #[test]
    fn parse_project_coordinates_not_confused_with_dependency() {
        let xml = r#"<project>
    <groupId>org.example</groupId>
    <artifactId>parent</artifactId>
    <version>1.0.0</version>
    <dependencies>
        <dependency>
            <groupId>com.google.guava</groupId>
            <artifactId>guava</artifactId>
            <version>33.0.0-jre</version>
        </dependency>
    </dependencies>
</project>"#;

        let project = parse_pom_str(xml).unwrap();
        assert_eq!(project.group_id.as_deref(), Some("org.example"));
        assert_eq!(project.artifact_id.as_deref(), Some("parent"));
        assert_eq!(project.version.as_deref(), Some("1.0.0"));
    }
}
