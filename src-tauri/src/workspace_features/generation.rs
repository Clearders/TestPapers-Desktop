//! Deterministic paper generation primitives, compatible with the existing backend fitness model.

use super::paper::{Difficulty, QuestionSnapshot, QuestionType};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionTypeTarget {
    #[serde(rename = "questionType")]
    pub(crate) question_type: QuestionType,
    pub(crate) count: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GenerationRequest {
    pub(crate) total_marks: u32,
    pub(crate) difficulty_coefficient: f64,
    pub(crate) question_types: Vec<QuestionTypeTarget>,
    pub(crate) subjects: Vec<String>,
    #[serde(default)]
    pub(crate) required_tags: Vec<String>,
    #[serde(default)]
    pub(crate) preferred_tags: Vec<String>,
    pub(crate) random_seed: u64,
    #[serde(default)]
    pub(crate) options: GeneticOptions,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeneticOptions {
    pub(crate) population_size: usize,
    pub(crate) generations: usize,
    pub(crate) crossover_rate: f64,
    pub(crate) mutation_rate: f64,
    pub(crate) elitism_count: usize,
    pub(crate) tournament_size: usize,
}

impl Default for GeneticOptions {
    fn default() -> Self {
        Self {
            population_size: 80,
            generations: 120,
            crossover_rate: 0.85,
            mutation_rate: 0.08,
            elitism_count: 4,
            tournament_size: 3,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TypeShortage {
    #[serde(rename = "type")]
    pub(crate) question_type: QuestionType,
    pub(crate) requested: u32,
    pub(crate) available: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GenerationDiagnostics {
    pub(crate) candidate_count: usize,
    pub(crate) question_count: usize,
    pub(crate) fitness: Option<f64>,
    pub(crate) generations_run: usize,
    pub(crate) marks_actual: u32,
    pub(crate) score_weight_actual: f64,
    pub(crate) difficulty_targets: BTreeMap<Difficulty, u32>,
    pub(crate) difficulty_actual: BTreeMap<Difficulty, u32>,
    pub(crate) type_targets: BTreeMap<QuestionType, u32>,
    pub(crate) type_actual: BTreeMap<QuestionType, u32>,
    pub(crate) shortages: Vec<TypeShortage>,
    pub(crate) missing_required_tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GeneratedQuestion {
    pub(crate) question: QuestionSnapshot,
    pub(crate) marks: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GenerationResult {
    pub(crate) selected: Vec<GeneratedQuestion>,
    pub(crate) diagnostics: GenerationDiagnostics,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum GenerationError {
    Invalid(&'static str),
    DuplicateCandidate(String),
    Insufficient(Box<GenerationDiagnostics>),
    Cancelled,
}

impl fmt::Display for GenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::DuplicateCandidate(id) => {
                write!(formatter, "duplicate generation candidate {id}")
            }
            Self::Insufficient(_) => formatter.write_str("not enough matching questions"),
            Self::Cancelled => formatter.write_str("generation was cancelled"),
        }
    }
}

pub(crate) trait GenerationObserver: Send + Sync {
    fn is_cancelled(&self) -> bool;
    fn progress(&self, completed_generations: usize, total_generations: usize);
}

pub(crate) struct NoopGenerationObserver;

impl GenerationObserver for NoopGenerationObserver {
    fn is_cancelled(&self) -> bool {
        false
    }

    fn progress(&self, _completed_generations: usize, _total_generations: usize) {}
}

pub(crate) fn generate(
    request: &GenerationRequest,
    candidates: &[QuestionSnapshot],
    observer: &dyn GenerationObserver,
) -> Result<GenerationResult, GenerationError> {
    validate_request(request)?;
    let requested_subjects = normalized_set(&request.subjects);
    let required_tags = normalized_set(&request.required_tags);
    let preferred_tags = normalized_set(&request.preferred_tags)
        .difference(&required_tags)
        .cloned()
        .collect::<BTreeSet<_>>();
    let type_targets = combined_type_targets(&request.question_types);
    let question_count = type_targets.values().sum::<u32>() as usize;
    if request.total_marks < question_count as u32 {
        return Err(GenerationError::Invalid(
            "total marks must be at least the selected question count",
        ));
    }

    let mut filtered = candidates
        .iter()
        .filter(|question| {
            type_targets.contains_key(&question.question_type)
                && question
                    .subjects
                    .iter()
                    .any(|subject| requested_subjects.contains(&normalize(subject)))
        })
        .cloned()
        .collect::<Vec<_>>();
    filtered.sort_by(|left, right| left.id.cmp(&right.id));
    for pair in filtered.windows(2) {
        if pair[0].id == pair[1].id {
            return Err(GenerationError::DuplicateCandidate(pair[0].id.clone()));
        }
    }

    let mut shortages = Vec::new();
    for (question_type, requested) in &type_targets {
        let available = filtered
            .iter()
            .filter(|question| question.question_type == *question_type)
            .count() as u32;
        if available < *requested {
            shortages.push(TypeShortage {
                question_type: *question_type,
                requested: *requested,
                available,
            });
        }
    }
    let all_tags = filtered
        .iter()
        .flat_map(|question| question.tags.iter().map(|tag| normalize(tag)))
        .collect::<BTreeSet<_>>();
    let missing_required_tags = required_tags
        .difference(&all_tags)
        .cloned()
        .collect::<Vec<_>>();
    let difficulty_targets =
        difficulty_targets(request.difficulty_coefficient, question_count as u32);
    if !shortages.is_empty() || !missing_required_tags.is_empty() {
        return Err(GenerationError::Insufficient(Box::new(empty_diagnostics(
            filtered.len(),
            question_count,
            difficulty_targets,
            type_targets,
            shortages,
            missing_required_tags,
        ))));
    }
    if observer.is_cancelled() {
        return Err(GenerationError::Cancelled);
    }

    let feature_tags = filtered
        .iter()
        .map(|question| normalized_set(&question.tags))
        .collect::<Vec<_>>();
    let feature_subjects = filtered
        .iter()
        .map(|question| normalized_set(&question.subjects))
        .collect::<Vec<_>>();
    let weights = filtered
        .iter()
        .map(|question| {
            question
                .score_weight
                .parse::<f64>()
                .unwrap_or(1.0)
                .max(0.01)
        })
        .collect::<Vec<_>>();
    let by_type = candidates_by_type(&filtered);
    let options = request.options;
    let population_size = options
        .population_size
        .max(options.elitism_count + options.tournament_size)
        .min(filtered.len().saturating_mul(8).max(1));
    let mut rng = DeterministicRng::new(request.random_seed);
    let mut population = (0..population_size)
        .map(|_| sample_individual(&by_type, &type_targets, &mut rng))
        .collect::<Vec<_>>();

    let mut best: Option<(f64, Vec<usize>)> = None;
    let mut no_improvement = 0_usize;
    let mut generations_run = 0_usize;
    for generation in 0..options.generations {
        if observer.is_cancelled() {
            return Err(GenerationError::Cancelled);
        }
        generations_run = generation + 1;
        let mut ranked = population
            .into_iter()
            .map(|individual| {
                let score = fitness(
                    &individual,
                    &filtered,
                    &feature_tags,
                    &feature_subjects,
                    &weights,
                    &difficulty_targets,
                    &type_targets,
                    &required_tags,
                    &preferred_tags,
                    request.total_marks as f64,
                );
                (score, individual)
            })
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| {
            right
                .0
                .total_cmp(&left.0)
                .then_with(|| compare_individuals(&left.1, &right.1, &filtered))
        });
        let improved = best.as_ref().is_none_or(|(score, _)| ranked[0].0 > *score);
        if improved {
            best = Some((ranked[0].0, ranked[0].1.clone()));
            no_improvement = 0;
        } else {
            no_improvement += 1;
        }
        observer.progress(generations_run, options.generations);
        if no_improvement >= 30 && generation >= 50 {
            break;
        }

        let elite_count = options.elitism_count.min(ranked.len());
        let mut next = ranked
            .iter()
            .take(elite_count)
            .map(|(_, individual)| individual.clone())
            .collect::<Vec<_>>();
        while next.len() < population_size {
            let first = tournament(&ranked, options.tournament_size, &mut rng);
            let second = tournament(&ranked, options.tournament_size, &mut rng);
            let mut child = if rng.next_f64() < options.crossover_rate {
                crossover(first, second, &filtered, &by_type, &type_targets, &mut rng)
            } else {
                first.clone()
            };
            mutate(
                &mut child,
                &filtered,
                &by_type,
                options.mutation_rate,
                &mut rng,
            );
            next.push(child);
        }
        population = next;
    }

    let (best_score, best) = best.unwrap_or_else(|| {
        let individual = sample_individual(&by_type, &type_targets, &mut rng);
        let score = fitness(
            &individual,
            &filtered,
            &feature_tags,
            &feature_subjects,
            &weights,
            &difficulty_targets,
            &type_targets,
            &required_tags,
            &preferred_tags,
            request.total_marks as f64,
        );
        (score, individual)
    });
    let selected_questions = best
        .iter()
        .map(|index| filtered[*index].clone())
        .collect::<Vec<_>>();
    let marks = distribute_marks(&selected_questions, request.total_marks)?;
    let mut difficulty_actual = BTreeMap::new();
    let mut type_actual = BTreeMap::new();
    for question in &selected_questions {
        *difficulty_actual.entry(question.difficulty).or_insert(0) += 1;
        *type_actual.entry(question.question_type).or_insert(0) += 1;
    }
    let score_weight_actual = selected_questions
        .iter()
        .map(|question| {
            question
                .score_weight
                .parse::<f64>()
                .unwrap_or(1.0)
                .max(0.01)
        })
        .sum();
    Ok(GenerationResult {
        selected: selected_questions
            .into_iter()
            .zip(marks)
            .map(|(question, marks)| GeneratedQuestion { question, marks })
            .collect(),
        diagnostics: GenerationDiagnostics {
            candidate_count: filtered.len(),
            question_count,
            fitness: Some((best_score * 100.0).round() / 100.0),
            generations_run,
            marks_actual: request.total_marks,
            score_weight_actual,
            difficulty_targets,
            difficulty_actual,
            type_targets,
            type_actual,
            shortages: vec![],
            missing_required_tags: vec![],
        },
    })
}

pub(crate) fn difficulty_targets(coefficient: f64, count: u32) -> BTreeMap<Difficulty, u32> {
    let coefficient = coefficient.clamp(0.0, 1.0);
    let anchors = [
        (Difficulty::Easy, 0.0),
        (Difficulty::Medium, 0.5),
        (Difficulty::Hard, 1.0),
    ];
    let weights = anchors.map(|(difficulty, anchor)| {
        (
            difficulty,
            (1.0_f64 - (coefficient - anchor).abs() * 2.0).max(0.0),
        )
    });
    let weight_total = weights
        .iter()
        .map(|(_, weight)| weight)
        .sum::<f64>()
        .max(f64::EPSILON);
    let mut targets = weights
        .iter()
        .map(|(difficulty, weight)| {
            (
                *difficulty,
                (count as f64 * weight / weight_total).round() as u32,
            )
        })
        .collect::<BTreeMap<_, _>>();
    normalize_counts(&mut targets, count);
    targets
}

pub(crate) fn distribute_marks(
    questions: &[QuestionSnapshot],
    total_marks: u32,
) -> Result<Vec<u32>, GenerationError> {
    if questions.is_empty() {
        return Ok(vec![]);
    }
    if total_marks < questions.len() as u32 {
        return Err(GenerationError::Invalid(
            "total marks must be at least the selected question count",
        ));
    }
    let weights = questions
        .iter()
        .map(|question| {
            question
                .score_weight
                .parse::<f64>()
                .unwrap_or(1.0)
                .max(0.01)
        })
        .collect::<Vec<_>>();
    let total_weight = weights.iter().sum::<f64>().max(f64::EPSILON);
    let raw = weights
        .iter()
        .map(|weight| (total_marks as f64 * weight / total_weight).max(1.0))
        .collect::<Vec<_>>();
    let mut marks = raw
        .iter()
        .map(|value| value.floor() as u32)
        .collect::<Vec<_>>();
    let mut remaining =
        i64::from(total_marks) - marks.iter().map(|value| i64::from(*value)).sum::<i64>();
    let mut ranked = (0..raw.len()).collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        let left_fraction = raw[*left] - raw[*left].floor();
        let right_fraction = raw[*right] - raw[*right].floor();
        right_fraction
            .total_cmp(&left_fraction)
            .then_with(|| left.cmp(right))
    });
    let mut cursor = 0;
    while remaining != 0 {
        let index = ranked[cursor % ranked.len()];
        if remaining > 0 {
            marks[index] += 1;
            remaining -= 1;
        } else if marks[index] > 1 {
            marks[index] -= 1;
            remaining += 1;
        }
        cursor += 1;
    }
    Ok(marks)
}

fn validate_request(request: &GenerationRequest) -> Result<(), GenerationError> {
    if request
        .subjects
        .iter()
        .all(|subject| subject.trim().is_empty())
    {
        return Err(GenerationError::Invalid("at least one subject is required"));
    }
    if !request.difficulty_coefficient.is_finite()
        || !(0.0..=1.0).contains(&request.difficulty_coefficient)
    {
        return Err(GenerationError::Invalid(
            "difficulty coefficient must be between 0 and 1",
        ));
    }
    if request.question_types.is_empty()
        || request
            .question_types
            .iter()
            .any(|target| target.count == 0)
    {
        return Err(GenerationError::Invalid(
            "question type counts must be positive",
        ));
    }
    let options = request.options;
    if options.population_size == 0
        || options.generations == 0
        || options.elitism_count == 0
        || options.tournament_size == 0
        || !(0.0..=1.0).contains(&options.crossover_rate)
        || !(0.0..=1.0).contains(&options.mutation_rate)
    {
        return Err(GenerationError::Invalid(
            "invalid genetic algorithm options",
        ));
    }
    Ok(())
}

fn combined_type_targets(targets: &[QuestionTypeTarget]) -> BTreeMap<QuestionType, u32> {
    let mut combined = BTreeMap::new();
    for target in targets {
        *combined.entry(target.question_type).or_insert(0) += target.count;
    }
    combined
}

fn candidates_by_type(candidates: &[QuestionSnapshot]) -> BTreeMap<QuestionType, Vec<usize>> {
    let mut result = BTreeMap::new();
    for (index, candidate) in candidates.iter().enumerate() {
        result
            .entry(candidate.question_type)
            .or_insert_with(Vec::new)
            .push(index);
    }
    result
}

fn sample_individual(
    by_type: &BTreeMap<QuestionType, Vec<usize>>,
    targets: &BTreeMap<QuestionType, u32>,
    rng: &mut DeterministicRng,
) -> Vec<usize> {
    let mut selected = Vec::new();
    for (question_type, count) in targets {
        let mut pool = by_type[question_type].clone();
        rng.shuffle(&mut pool);
        selected.extend(pool.into_iter().take(*count as usize));
    }
    rng.shuffle(&mut selected);
    selected
}

#[allow(clippy::too_many_arguments)]
fn fitness(
    individual: &[usize],
    candidates: &[QuestionSnapshot],
    tags_by_candidate: &[BTreeSet<String>],
    subjects_by_candidate: &[BTreeSet<String>],
    weights: &[f64],
    difficulty_targets: &BTreeMap<Difficulty, u32>,
    type_targets: &BTreeMap<QuestionType, u32>,
    required_tags: &BTreeSet<String>,
    preferred_tags: &BTreeSet<String>,
    target_weight: f64,
) -> f64 {
    let mut difficulty_actual = BTreeMap::new();
    let mut type_actual = BTreeMap::new();
    let mut tags = BTreeSet::new();
    let mut subjects = BTreeSet::new();
    let mut score_weight = 0.0;
    for index in individual {
        let candidate = &candidates[*index];
        *difficulty_actual
            .entry(candidate.difficulty)
            .or_insert(0_u32) += 1;
        *type_actual.entry(candidate.question_type).or_insert(0_u32) += 1;
        tags.extend(tags_by_candidate[*index].iter().cloned());
        subjects.extend(subjects_by_candidate[*index].iter().cloned());
        score_weight += weights[*index];
    }
    let difficulty_penalty = difficulty_targets
        .iter()
        .map(|(difficulty, target)| {
            target.abs_diff(*difficulty_actual.get(difficulty).unwrap_or(&0)) as f64 * 40.0
        })
        .sum::<f64>();
    let type_penalty = type_targets
        .iter()
        .map(|(question_type, target)| {
            target.abs_diff(*type_actual.get(question_type).unwrap_or(&0)) as f64 * 30.0
        })
        .sum::<f64>();
    let required_penalty = required_tags.difference(&tags).count() as f64 * 80.0;
    let weight_penalty = (score_weight - target_weight).abs() * 8.0;
    let preferred_bonus = preferred_tags.intersection(&tags).count() as f64 * 24.0;
    let diversity_bonus = tags.len().min(10) as f64 * 2.0 + subjects.len().min(3) as f64 * 3.0;
    1000.0 - difficulty_penalty - type_penalty - required_penalty - weight_penalty
        + preferred_bonus
        + diversity_bonus
}

fn crossover(
    first: &[usize],
    second: &[usize],
    candidates: &[QuestionSnapshot],
    by_type: &BTreeMap<QuestionType, Vec<usize>>,
    targets: &BTreeMap<QuestionType, u32>,
    rng: &mut DeterministicRng,
) -> Vec<usize> {
    let pivot = if first.len() > 1 {
        rng.usize(first.len() - 1) + 1
    } else {
        1
    };
    let mut source = first
        .iter()
        .take(pivot)
        .chain(second)
        .copied()
        .collect::<Vec<_>>();
    for pool in by_type.values() {
        source.extend(pool.iter().copied());
    }
    let mut selected = BTreeSet::new();
    let mut counts = BTreeMap::<QuestionType, u32>::new();
    let mut child = Vec::new();
    for index in source {
        let question_type = candidates[index].question_type;
        if selected.contains(&index)
            || counts.get(&question_type).copied().unwrap_or(0) >= targets[&question_type]
        {
            continue;
        }
        selected.insert(index);
        *counts.entry(question_type).or_insert(0) += 1;
        child.push(index);
    }
    child
}

fn mutate(
    individual: &mut [usize],
    candidates: &[QuestionSnapshot],
    by_type: &BTreeMap<QuestionType, Vec<usize>>,
    rate: f64,
    rng: &mut DeterministicRng,
) {
    let mut selected = individual.iter().copied().collect::<BTreeSet<_>>();
    for current in individual {
        if rng.next_f64() >= rate {
            continue;
        }
        let pool = &by_type[&candidates[*current].question_type];
        let available = pool
            .iter()
            .copied()
            .filter(|candidate| !selected.contains(candidate))
            .collect::<Vec<_>>();
        if available.is_empty() {
            continue;
        }
        let replacement = available[rng.usize(available.len())];
        selected.remove(current);
        selected.insert(replacement);
        *current = replacement;
    }
}

fn tournament<'a>(
    ranked: &'a [(f64, Vec<usize>)],
    size: usize,
    rng: &mut DeterministicRng,
) -> &'a Vec<usize> {
    let mut best_index = rng.usize(ranked.len());
    for _ in 1..size.min(ranked.len()) {
        let candidate = rng.usize(ranked.len());
        if ranked[candidate].0 > ranked[best_index].0 {
            best_index = candidate;
        }
    }
    &ranked[best_index].1
}

fn compare_individuals(
    left: &[usize],
    right: &[usize],
    candidates: &[QuestionSnapshot],
) -> std::cmp::Ordering {
    left.iter()
        .map(|index| &candidates[*index].id)
        .cmp(right.iter().map(|index| &candidates[*index].id))
}

fn normalize_counts<T: Ord + Copy>(counts: &mut BTreeMap<T, u32>, target: u32) {
    while counts.values().sum::<u32>() < target {
        let key = *counts
            .iter()
            .max_by_key(|(_, value)| *value)
            .expect("non-empty counts")
            .0;
        *counts.get_mut(&key).expect("key exists") += 1;
    }
    while counts.values().sum::<u32>() > target {
        let key = *counts
            .iter()
            .filter(|(_, value)| **value > 0)
            .max_by_key(|(_, value)| *value)
            .expect("positive count")
            .0;
        *counts.get_mut(&key).expect("key exists") -= 1;
    }
}

fn empty_diagnostics(
    candidate_count: usize,
    question_count: usize,
    difficulty_targets: BTreeMap<Difficulty, u32>,
    type_targets: BTreeMap<QuestionType, u32>,
    shortages: Vec<TypeShortage>,
    missing_required_tags: Vec<String>,
) -> GenerationDiagnostics {
    GenerationDiagnostics {
        candidate_count,
        question_count,
        fitness: None,
        generations_run: 0,
        marks_actual: 0,
        score_weight_actual: 0.0,
        difficulty_targets,
        difficulty_actual: BTreeMap::new(),
        type_targets,
        type_actual: BTreeMap::new(),
        shortages,
        missing_required_tags,
    }
}

fn normalized_set(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| normalize(value))
        .filter(|value| !value.is_empty())
        .collect()
}

fn normalize(value: &str) -> String {
    value.trim().to_lowercase()
}

struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9e37_79b9_7f4a_7c15,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn usize(&mut self, upper_exclusive: usize) -> usize {
        debug_assert!(upper_exclusive > 0);
        (self.next_u64() % upper_exclusive as u64) as usize
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / ((1_u64 << 53) as f64))
    }

    fn shuffle<T>(&mut self, values: &mut [T]) {
        for index in (1..values.len()).rev() {
            values.swap(index, self.usize(index + 1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_features::paper::{AnswerSnapshot, EssayBlankSpace};

    fn question(
        index: u32,
        question_type: QuestionType,
        difficulty: Difficulty,
        tags: &[&str],
    ) -> QuestionSnapshot {
        QuestionSnapshot {
            id: format!("018f0000-0000-7000-8000-{index:012}"),
            version: 1,
            content_hash: format!("{index:064x}"),
            question_type,
            subjects: vec!["Math".into()],
            difficulty,
            tags: tags.iter().map(|tag| (*tag).into()).collect(),
            text: format!("Question {index}"),
            options: None,
            answer: AnswerSnapshot::Text("answer".into()),
            has_latex: false,
            source: None,
            essay_blank_space: (question_type == QuestionType::Essay)
                .then_some(EssayBlankSpace { lines: 6 }),
            score_weight: if difficulty == Difficulty::Hard {
                "2".into()
            } else {
                "1".into()
            },
            attachments: vec![],
        }
    }

    fn request() -> GenerationRequest {
        GenerationRequest {
            total_marks: 10,
            difficulty_coefficient: 0.5,
            question_types: vec![
                QuestionTypeTarget {
                    question_type: QuestionType::SingleChoice,
                    count: 2,
                },
                QuestionTypeTarget {
                    question_type: QuestionType::ShortAnswer,
                    count: 2,
                },
            ],
            subjects: vec!["Math".into()],
            required_tags: vec!["core".into()],
            preferred_tags: vec!["algebra".into()],
            random_seed: 42,
            options: GeneticOptions {
                population_size: 16,
                generations: 20,
                ..GeneticOptions::default()
            },
        }
    }

    #[test]
    fn seeded_generation_is_independent_of_input_order() {
        let mut candidates = vec![
            question(1, QuestionType::SingleChoice, Difficulty::Easy, &["core"]),
            question(
                2,
                QuestionType::SingleChoice,
                Difficulty::Medium,
                &["algebra"],
            ),
            question(3, QuestionType::SingleChoice, Difficulty::Hard, &[]),
            question(4, QuestionType::ShortAnswer, Difficulty::Easy, &[]),
            question(5, QuestionType::ShortAnswer, Difficulty::Medium, &["core"]),
            question(6, QuestionType::ShortAnswer, Difficulty::Hard, &["algebra"]),
        ];
        let first = generate(&request(), &candidates, &NoopGenerationObserver).unwrap();
        candidates.reverse();
        let second = generate(&request(), &candidates, &NoopGenerationObserver).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.selected.iter().map(|item| item.marks).sum::<u32>(),
            10
        );
        assert_eq!(
            first.diagnostics.type_actual,
            first.diagnostics.type_targets
        );
    }

    #[test]
    fn reports_type_and_required_tag_shortages_without_partial_result() {
        let candidates = vec![question(
            1,
            QuestionType::SingleChoice,
            Difficulty::Easy,
            &[],
        )];
        let error = generate(&request(), &candidates, &NoopGenerationObserver).unwrap_err();
        let GenerationError::Insufficient(diagnostics) = error else {
            panic!("expected diagnostics")
        };
        assert_eq!(diagnostics.shortages.len(), 2);
        assert_eq!(diagnostics.missing_required_tags, vec!["core"]);
    }

    #[test]
    fn mark_distribution_is_positive_and_exact() {
        let questions = vec![
            question(1, QuestionType::Essay, Difficulty::Hard, &[]),
            question(2, QuestionType::ShortAnswer, Difficulty::Easy, &[]),
            question(3, QuestionType::ShortAnswer, Difficulty::Easy, &[]),
        ];
        let marks = distribute_marks(&questions, 11).unwrap();
        assert_eq!(marks.iter().sum::<u32>(), 11);
        assert!(marks.iter().all(|marks| *marks >= 1));
        assert!(marks[0] > marks[1]);
    }
}
