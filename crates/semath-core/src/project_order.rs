use std::collections::{HashMap, HashSet};

use crate::ProjectInclude;

#[derive(Clone, Debug)]
pub(crate) struct ProjectOrderDocument {
    pub file_id: String,
    pub includes: Vec<ProjectInclude>,
    pub occurrence_offsets: Vec<u32>,
    pub path: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ProjectOrder {
    component_by_file: HashMap<String, String>,
    positions: HashMap<(String, u32), Option<u64>>,
}

impl ProjectOrder {
    pub fn new(documents: Vec<ProjectOrderDocument>, main_file_id: Option<&str>) -> Self {
        let by_id = documents
            .into_iter()
            .map(|document| (document.file_id.clone(), document))
            .collect::<HashMap<_, _>>();
        let path_to_id = by_id
            .values()
            .map(|document| {
                (
                    normalize_project_path(&document.path),
                    document.file_id.clone(),
                )
            })
            .collect::<HashMap<_, _>>();
        let directed = directed_edges(&by_id, &path_to_id);
        let component_by_file = components(&by_id, &directed, main_file_id);
        let positions = source_positions(&by_id, &directed, &component_by_file, main_file_id);
        Self {
            component_by_file,
            positions,
        }
    }

    pub fn component_for(&self, file_id: &str) -> Option<&str> {
        self.component_by_file.get(file_id).map(String::as_str)
    }

    pub fn precedes(
        &self,
        definition_file_id: &str,
        definition_offset: u32,
        occurrence_file_id: &str,
        occurrence_offset: u32,
    ) -> bool {
        let definition = self
            .positions
            .get(&(definition_file_id.to_string(), definition_offset))
            .copied()
            .flatten();
        let occurrence = self
            .positions
            .get(&(occurrence_file_id.to_string(), occurrence_offset))
            .copied()
            .flatten();
        matches!((definition, occurrence), (Some(left), Some(right)) if left <= right)
    }
}

#[derive(Clone, Debug)]
struct IncludeEdge {
    offset: u32,
    target: String,
}

fn directed_edges(
    documents: &HashMap<String, ProjectOrderDocument>,
    path_to_id: &HashMap<String, String>,
) -> HashMap<String, Vec<IncludeEdge>> {
    documents
        .values()
        .map(|document| {
            let mut edges = document
                .includes
                .iter()
                .filter_map(|include| {
                    resolve_include(&document.path, &include.path, path_to_id).map(|target| {
                        IncludeEdge {
                            offset: include.source_range.start_offset,
                            target,
                        }
                    })
                })
                .collect::<Vec<_>>();
            edges.sort_by(|left, right| {
                left.offset
                    .cmp(&right.offset)
                    .then(left.target.cmp(&right.target))
            });
            (document.file_id.clone(), edges)
        })
        .collect()
}

fn components(
    documents: &HashMap<String, ProjectOrderDocument>,
    directed: &HashMap<String, Vec<IncludeEdge>>,
    main_file_id: Option<&str>,
) -> HashMap<String, String> {
    let mut adjacency = documents
        .keys()
        .map(|file_id| (file_id.clone(), HashSet::new()))
        .collect::<HashMap<_, _>>();
    for (source, edges) in directed {
        for edge in edges {
            adjacency
                .entry(source.clone())
                .or_default()
                .insert(edge.target.clone());
            adjacency
                .entry(edge.target.clone())
                .or_default()
                .insert(source.clone());
        }
    }

    let mut component_by_file = HashMap::new();
    let mut remaining = documents.keys().cloned().collect::<HashSet<_>>();
    while let Some(seed) = remaining.iter().min().cloned() {
        let mut pending = vec![seed.clone()];
        let mut members = Vec::new();
        remaining.remove(&seed);
        while let Some(file_id) = pending.pop() {
            members.push(file_id.clone());
            let mut neighbors = adjacency
                .get(&file_id)
                .into_iter()
                .flatten()
                .cloned()
                .collect::<Vec<_>>();
            neighbors.sort();
            for neighbor in neighbors {
                if remaining.remove(&neighbor) {
                    pending.push(neighbor);
                }
            }
        }
        members.sort();
        let component_id = main_file_id
            .filter(|main| members.iter().any(|member| member == main))
            .map(str::to_owned)
            .unwrap_or_else(|| members[0].clone());
        for file_id in members {
            component_by_file.insert(file_id, component_id.clone());
        }
    }
    component_by_file
}

fn source_positions(
    documents: &HashMap<String, ProjectOrderDocument>,
    directed: &HashMap<String, Vec<IncludeEdge>>,
    component_by_file: &HashMap<String, String>,
    main_file_id: Option<&str>,
) -> HashMap<(String, u32), Option<u64>> {
    let mut roots_by_component: HashMap<String, Vec<String>> = HashMap::new();
    let mut incoming = HashMap::<String, usize>::new();
    for edges in directed.values() {
        for edge in edges {
            *incoming.entry(edge.target.clone()).or_default() += 1;
        }
    }
    for file_id in documents.keys() {
        let component = component_by_file[file_id].clone();
        let is_main = main_file_id == Some(file_id.as_str());
        if is_main || (main_file_id != Some(component.as_str()) && !incoming.contains_key(file_id))
        {
            roots_by_component
                .entry(component)
                .or_default()
                .push(file_id.clone());
        }
    }
    for (component, roots) in &mut roots_by_component {
        roots.sort();
        if main_file_id == Some(component.as_str()) {
            roots.retain(|root| root == component);
        }
    }

    let mut positions = HashMap::new();
    let mut sequence = 0_u64;
    let mut components = roots_by_component.into_iter().collect::<Vec<_>>();
    components.sort_by(|left, right| left.0.cmp(&right.0));
    for (_, roots) in components {
        for root in roots {
            visit(
                &root,
                documents,
                directed,
                &mut HashSet::new(),
                &mut positions,
                &mut sequence,
            );
        }
    }
    positions
}

fn visit(
    file_id: &str,
    documents: &HashMap<String, ProjectOrderDocument>,
    directed: &HashMap<String, Vec<IncludeEdge>>,
    active: &mut HashSet<String>,
    positions: &mut HashMap<(String, u32), Option<u64>>,
    sequence: &mut u64,
) {
    if !active.insert(file_id.to_string()) {
        return;
    }
    let Some(document) = documents.get(file_id) else {
        active.remove(file_id);
        return;
    };
    let mut events = document
        .occurrence_offsets
        .iter()
        .copied()
        .map(Event::Occurrence)
        .chain(
            directed
                .get(file_id)
                .into_iter()
                .flatten()
                .cloned()
                .map(Event::Include),
        )
        .collect::<Vec<_>>();
    events.sort_by(|left, right| {
        left.offset()
            .cmp(&right.offset())
            .then(left.kind_order().cmp(&right.kind_order()))
    });
    events.dedup_by(|left, right| match (left, right) {
        (Event::Occurrence(left), Event::Occurrence(right)) => left == right,
        _ => false,
    });
    for event in events {
        match event {
            Event::Occurrence(offset) => {
                let key = (file_id.to_string(), offset);
                positions
                    .entry(key)
                    .and_modify(|position| *position = None)
                    .or_insert(Some(*sequence));
                *sequence += 1;
            }
            Event::Include(edge) => visit(
                &edge.target,
                documents,
                directed,
                active,
                positions,
                sequence,
            ),
        }
    }
    active.remove(file_id);
}

#[derive(Clone, Debug)]
enum Event {
    Occurrence(u32),
    Include(IncludeEdge),
}

impl Event {
    fn offset(&self) -> u32 {
        match self {
            Self::Occurrence(offset) => *offset,
            Self::Include(edge) => edge.offset,
        }
    }

    fn kind_order(&self) -> u8 {
        match self {
            Self::Occurrence(_) => 0,
            Self::Include(_) => 1,
        }
    }
}

fn resolve_include(
    source_path: &str,
    include_path: &str,
    path_to_id: &HashMap<String, String>,
) -> Option<String> {
    let parent = source_path
        .rsplit_once('/')
        .map_or("", |(parent, _)| parent);
    let joined = if include_path.starts_with('/') || parent.is_empty() {
        include_path.trim_start_matches('/').to_string()
    } else {
        format!("{parent}/{include_path}")
    };
    let normalized = normalize_project_path(&joined);
    path_to_id.get(&normalized).cloned().or_else(|| {
        (!normalized.contains('.'))
            .then(|| path_to_id.get(&format!("{normalized}.tex")).cloned())
            .flatten()
    })
}

fn normalize_project_path(path: &str) -> String {
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::{ProjectOrder, ProjectOrderDocument};
    use crate::{ProjectInclude, SourceRange};

    fn document(
        file_id: &str,
        path: &str,
        occurrences: &[u32],
        includes: &[(&str, u32)],
    ) -> ProjectOrderDocument {
        ProjectOrderDocument {
            file_id: file_id.into(),
            includes: includes
                .iter()
                .map(|(path, offset)| ProjectInclude {
                    path: (*path).into(),
                    source_range: SourceRange {
                        start_offset: *offset,
                        end_offset: *offset + 1,
                    },
                })
                .collect(),
            occurrence_offsets: occurrences.to_vec(),
            path: path.into(),
        }
    }

    #[test]
    fn expands_nested_includes_at_their_source_position() {
        let order = ProjectOrder::new(
            vec![
                document("main", "main.tex", &[1, 9], &[("chapter", 5)]),
                document("chapter", "chapter.tex", &[2, 8], &[("parts/nested", 5)]),
                document("nested", "parts/nested.tex", &[3], &[]),
            ],
            Some("main"),
        );

        assert!(order.precedes("main", 1, "nested", 3));
        assert!(order.precedes("nested", 3, "chapter", 8));
        assert!(order.precedes("chapter", 8, "main", 9));
        assert!(!order.precedes("main", 9, "chapter", 2));
    }

    #[test]
    fn resolves_parent_relative_paths_and_separates_components() {
        let order = ProjectOrder::new(
            vec![
                document("main", "book/main.tex", &[1], &[("chapters/a", 2)]),
                document("a", "book/chapters/a.tex", &[1], &[("../../shared/b", 2)]),
                document("b", "shared/b.tex", &[1], &[]),
                document("orphan", "notes/orphan.tex", &[1], &[]),
            ],
            Some("main"),
        );

        assert_eq!(order.component_for("a"), Some("main"));
        assert_eq!(order.component_for("b"), Some("main"));
        assert_eq!(order.component_for("orphan"), Some("orphan"));
    }

    #[test]
    fn refuses_source_order_when_a_file_is_included_more_than_once() {
        let order = ProjectOrder::new(
            vec![
                document(
                    "main",
                    "main.tex",
                    &[1, 9],
                    &[("chapter", 3), ("chapter", 7)],
                ),
                document("chapter", "chapter.tex", &[2, 4], &[]),
            ],
            Some("main"),
        );

        assert!(!order.precedes("chapter", 2, "main", 9));
        assert!(!order.precedes("main", 1, "chapter", 4));
    }
}
