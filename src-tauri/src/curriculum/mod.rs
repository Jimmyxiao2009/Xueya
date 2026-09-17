//! Deterministic curriculum planning.
//!
//! Everything in this module is pure: given a [`RouteBrief`] it produces the
//! same roadmap, lesson plan and quiz every time. It is the fallback used when
//! there is no AI provider configured (or when a topic has no local evidence),
//! and it is also the shape the AI provider is asked to reproduce.
//!
//! The six-node roadmap mirrors the Orbit prototype's two demo routes so the
//! product looks identical with or without a model.

use crate::domain::NodeKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopicKind {
    Geometry,
    Algebra,
    Custom,
}

impl TopicKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Geometry => "geometry",
            Self::Algebra => "algebra",
            Self::Custom => "custom",
        }
    }
}

/// Keyword detection, ported from the prototype's `detectTopicKind`.
pub fn detect_topic_kind(topic: &str) -> TopicKind {
    let lowered = topic.to_lowercase();
    if lowered.contains("线性")
        || lowered.contains("linear")
        || lowered.contains("矩阵")
        || lowered.contains("向量")
        || lowered.contains("vector")
    {
        return TopicKind::Algebra;
    }
    if lowered.contains("射影") || lowered.contains("projective") {
        return TopicKind::Geometry;
    }
    TopicKind::Custom
}

#[derive(Debug, Clone)]
pub struct RouteBrief {
    pub topic: String,
    pub goal: Option<String>,
    /// beginner | some_experience | advanced
    pub experience_level: Option<String>,
    /// overview | practical | deep
    pub desired_depth: Option<String>,
}

impl RouteBrief {
    pub fn new(topic: impl Into<String>) -> Self {
        Self {
            topic: topic.into(),
            goal: None,
            experience_level: None,
            desired_depth: None,
        }
    }

    pub fn depth(&self) -> &str {
        match self.desired_depth.as_deref() {
            Some("overview") => "overview",
            Some("deep") => "deep",
            _ => "practical",
        }
    }

    pub fn experience(&self) -> &str {
        match self.experience_level.as_deref() {
            Some("advanced") => "advanced",
            Some("beginner") => "beginner",
            _ => "some_experience",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanOrigin {
    Deterministic,
    Ai,
}

impl PlanOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::Ai => "ai",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlannedNode {
    pub key: String,
    pub title: String,
    pub objective: String,
    pub description: String,
    pub concepts: Vec<String>,
    pub dependencies: Vec<String>,
    pub kind: NodeKind,
    pub estimated_minutes: i64,
}

#[derive(Debug, Clone)]
pub struct PlannedRoute {
    pub title: String,
    pub description: String,
    pub learning_objectives: Vec<String>,
    pub prerequisites: Vec<String>,
    pub nodes: Vec<PlannedNode>,
    pub origin: PlanOrigin,
}

struct NodeTemplate {
    title: &'static str,
    objective: &'static str,
    concepts: &'static [&'static str],
    kind: NodeKind,
    minutes: i64,
}

const GEOMETRY: &[NodeTemplate] = &[
    NodeTemplate {
        title: "从欧氏到射影",
        objective: "理解欧氏平面为什么不够用，以及射影平面补上了什么。",
        concepts: &["欧氏平面", "平行公设", "射影平面"],
        kind: NodeKind::Lesson,
        minutes: 12,
    },
    NodeTemplate {
        title: "无穷远点",
        objective: "把「方向」本身变成一个点，并说明它与有限点的区别。",
        concepts: &["方向", "无穷远点", "点的扩充"],
        kind: NodeKind::Lesson,
        minutes: 17,
    },
    NodeTemplate {
        title: "方向与无穷远点",
        objective: "巩固方向与无穷远点的一一对应关系。",
        concepts: &["平行线族", "方向对应"],
        kind: NodeKind::Review,
        minutes: 8,
    },
    NodeTemplate {
        title: "无穷远直线",
        objective: "把全部无穷远点放在一起，理解它构成一条直线。",
        concepts: &["无穷远直线", "齐次方程"],
        kind: NodeKind::Lesson,
        minutes: 15,
    },
    NodeTemplate {
        title: "第一阶段 · 小试身手",
        objective: "用一组检查题确认前四个节点是否真正理解。",
        concepts: &["概念辨析", "坐标判断"],
        kind: NodeKind::Quiz,
        minutes: 12,
    },
    NodeTemplate {
        title: "交比",
        objective: "认识射影变换下的不变量，并会用有向坐标计算。",
        concepts: &["交比", "射影不变量"],
        kind: NodeKind::Lesson,
        minutes: 18,
    },
];

const ALGEBRA: &[NodeTemplate] = &[
    NodeTemplate {
        title: "向量与空间",
        objective: "把向量当作箭头，建立几何直觉。",
        concepts: &["向量", "空间", "几何直觉"],
        kind: NodeKind::Lesson,
        minutes: 12,
    },
    NodeTemplate {
        title: "线性组合",
        objective: "理解数乘与加法如何把两个向量组合成新的向量。",
        concepts: &["数乘", "加法", "线性组合"],
        kind: NodeKind::Lesson,
        minutes: 15,
    },
    NodeTemplate {
        title: "向量与线性组合",
        objective: "巩固系数对向量组合的影响。",
        concepts: &["系数", "张成空间"],
        kind: NodeKind::Review,
        minutes: 8,
    },
    NodeTemplate {
        title: "线性无关",
        objective: "判断一组向量是否互相独立，理解共线与张成的关系。",
        concepts: &["线性无关", "共线", "张成"],
        kind: NodeKind::Lesson,
        minutes: 15,
    },
    NodeTemplate {
        title: "第一阶段 · 小试身手",
        objective: "用一组检查题确认前四个节点是否真正理解。",
        concepts: &["概念辨析", "系数判断"],
        kind: NodeKind::Quiz,
        minutes: 12,
    },
    NodeTemplate {
        title: "基与维数",
        objective: "用基描述一个空间，并理解维数的含义。",
        concepts: &["基", "维数", "坐标"],
        kind: NodeKind::Lesson,
        minutes: 18,
    },
];

const CUSTOM: &[NodeTemplate] = &[
    NodeTemplate {
        title: "基础概念",
        objective: "先把这门主题的关键名词与边界说清楚。",
        concepts: &["关键名词", "边界"],
        kind: NodeKind::Lesson,
        minutes: 12,
    },
    NodeTemplate {
        title: "建立核心直觉",
        objective: "用一个具体例子建立可以复述的直觉。",
        concepts: &["具体例子", "核心直觉"],
        kind: NodeKind::Lesson,
        minutes: 15,
    },
    NodeTemplate {
        title: "关键概念复习",
        objective: "把前面两个节点的薄弱点重新过一遍。",
        concepts: &["回顾", "巩固"],
        kind: NodeKind::Review,
        minutes: 8,
    },
    NodeTemplate {
        title: "方法与应用",
        objective: "把概念用到一道真实问题上。",
        concepts: &["方法", "应用"],
        kind: NodeKind::Lesson,
        minutes: 15,
    },
    NodeTemplate {
        title: "第一阶段 · 小试身手",
        objective: "用一组检查题确认前四个节点是否真正理解。",
        concepts: &["概念辨析", "应用判断"],
        kind: NodeKind::Quiz,
        minutes: 12,
    },
    NodeTemplate {
        title: "综合实践",
        objective: "完成一个可以把知识用起来的综合任务。",
        concepts: &["综合", "实践"],
        kind: NodeKind::Practice,
        minutes: 18,
    },
];

fn templates(kind: TopicKind) -> &'static [NodeTemplate] {
    match kind {
        TopicKind::Geometry => GEOMETRY,
        TopicKind::Algebra => ALGEBRA,
        TopicKind::Custom => CUSTOM,
    }
}

fn strip_trailing_punctuation(value: &str) -> String {
    value
        .trim()
        .trim_end_matches(['。', '.', '！', '!', '？', '?'])
        .to_owned()
}

/// Build the deterministic roadmap.
pub fn plan_route(brief: &RouteBrief) -> PlannedRoute {
    let topic = strip_trailing_punctuation(&brief.topic);
    let kind = detect_topic_kind(&topic);
    let mut chosen: Vec<&NodeTemplate> = templates(kind).iter().collect();

    match brief.depth() {
        "overview" => {
            // Keep foundations + review + quiz: the shortest honest path.
            chosen = vec![chosen[0], chosen[1], chosen[2], chosen[4]];
        }
        "deep" => {
            chosen.push(&DEEP_LESSON);
            chosen.push(&DEEP_PROJECT);
        }
        _ => {}
    }

    if brief.experience() == "advanced" && chosen.len() > 3 {
        // An advanced learner skips the "what is this" node but keeps the
        // review node, because spaced repetition still applies.
        chosen.remove(0);
    }

    let mut nodes = Vec::with_capacity(chosen.len());
    for (index, template) in chosen.iter().enumerate() {
        let key = format!("n{}", index + 1);
        let dependencies = if index == 0 {
            Vec::new()
        } else {
            vec![format!("n{index}")]
        };
        let mut concepts: Vec<String> = template
            .concepts
            .iter()
            .map(|value| (*value).to_owned())
            .collect();
        if kind == TopicKind::Custom {
            concepts.push(topic.clone());
        }
        nodes.push(PlannedNode {
            key,
            title: template.title.to_owned(),
            objective: template.objective.to_owned(),
            description: format!("{}（{} 分钟）", template.objective, template.minutes),
            concepts,
            dependencies,
            kind: template.kind,
            estimated_minutes: template.minutes,
        });
    }

    let title = if topic.is_empty() {
        "新的学习路线".to_owned()
    } else {
        topic.clone()
    };
    let description = brief
        .goal
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(strip_trailing_punctuation)
        .unwrap_or_else(|| format!("从基础概念出发，逐步走到可以复述、可以应用的 {title}。"));

    let learning_objectives = nodes.iter().map(|node| node.objective.clone()).collect();

    PlannedRoute {
        title,
        description,
        learning_objectives,
        prerequisites: match brief.experience() {
            "advanced" => vec![],
            _ => vec!["高中数学与逻辑推理".to_owned()],
        },
        nodes,
        origin: PlanOrigin::Deterministic,
    }
}

const DEEP_LESSON: NodeTemplate = NodeTemplate {
    title: "进阶专题",
    objective: "把这一阶段的方法推进到更一般的情形。",
    concepts: &["进阶", "推广"],
    kind: NodeKind::Lesson,
    minutes: 20,
};

const DEEP_PROJECT: NodeTemplate = NodeTemplate {
    title: "综合项目",
    objective: "独立完成一个可以展示的项目，并解释其中的取舍。",
    concepts: &["项目", "取舍"],
    kind: NodeKind::Practice,
    minutes: 25,
};

/// Deterministic lesson body used when there is no AI provider or no evidence.
#[derive(Debug, Clone)]
pub struct PlannedLesson {
    pub title: String,
    pub explanation: String,
    pub key_points: Vec<String>,
    pub diagnostic_question: String,
}

pub fn plan_lesson(
    node_title: &str,
    node_objective: &str,
    concepts: &[String],
    topic: &str,
) -> PlannedLesson {
    let concept_line = if concepts.is_empty() {
        "这一节的关键词会随着你的回答逐步补全。".to_owned()
    } else {
        format!("这一节会围绕 {} 展开。", concepts.join("、"))
    };

    let explanation = format!(
        "先把这一节的目标说清楚：{objective}\n\n{concept_line}\
         \n\n你可以按三步走：\n1. 用一句话说出「{title}」在 {topic} 里解决什么问题；\n\
         2. 找一个具体例子，把概念代进去算一遍或画一遍；\n\
         3. 合上资料，用自己的话把它讲出来。\n\n\
         讲不出来没关系——下面的检查题就是为了暴露断点，答错会触发路线插入加练节点。",
        objective = strip_trailing_punctuation(node_objective),
        title = node_title,
        topic = topic,
    );

    let key_points = if concepts.is_empty() {
        vec![
            format!("用一句话说清「{node_title}」要解决的问题"),
            "给出一个具体例子".to_owned(),
            "解释为什么这个结论成立".to_owned(),
        ]
    } else {
        concepts
            .iter()
            .map(|concept| format!("能解释「{concept}」并举例"))
            .collect()
    };

    PlannedLesson {
        title: node_title.to_owned(),
        explanation,
        key_points,
        diagnostic_question: format!(
            "请用自己的话解释：{objective}",
            objective = strip_trailing_punctuation(node_objective)
        ),
    }
}

/// One deterministic check set per node, in the five widget types the product
/// already supports.
pub fn plan_quiz(
    node_title: &str,
    node_objective: &str,
    concepts: &[String],
) -> Vec<PlannedQuestion> {
    let objective = strip_trailing_punctuation(node_objective);
    let primary = concepts
        .first()
        .cloned()
        .unwrap_or_else(|| node_title.to_owned());
    let secondary = concepts
        .get(1)
        .cloned()
        .unwrap_or_else(|| format!("{node_title} 的应用"));

    vec![
        PlannedQuestion {
            kind: crate::domain::QuizKind::Single,
            prompt: format!("关于「{node_title}」，下面哪一项是本课要达成的目标？"),
            options: vec![
                objective.clone(),
                format!("背下所有定义，不必理解它们之间的关系"),
                format!("跳过基础，直接学习 {secondary} 的证明细节"),
            ],
            answer: vec!["0".to_owned()],
            explanation: format!("本课的目标是：{objective}"),
            difficulty: 0.3,
        },
        PlannedQuestion {
            kind: crate::domain::QuizKind::Multi,
            prompt: "下面哪些说法符合本课的检查标准？".to_owned(),
            options: vec![
                format!("能解释「{primary}」并举例"),
                format!("能说出「{node_title}」解决的问题"),
                "只要记住结论即可，不需要说明理由".to_owned(),
                "可以跳过检查题直接进入下一节点".to_owned(),
            ],
            answer: vec!["0".to_owned(), "1".to_owned()],
            explanation: "理解的标准是能解释、能举例、能说出它解决的问题。".to_owned(),
            difficulty: 0.45,
        },
        PlannedQuestion {
            kind: crate::domain::QuizKind::Fill,
            prompt: "本节点标题是「____」。".to_owned(),
            options: vec![],
            answer: vec![node_title.to_owned()],
            explanation: format!("这一节点的标题是「{node_title}」。"),
            difficulty: 0.2,
        },
        PlannedQuestion {
            kind: crate::domain::QuizKind::Short,
            prompt: format!("用自己的话解释：{objective}"),
            options: vec![],
            answer: vec![],
            explanation: "简答题由导师模型评阅，本地引擎不会替你打分。".to_owned(),
            difficulty: 0.6,
        },
        PlannedQuestion {
            kind: crate::domain::QuizKind::Image,
            prompt: format!("画出你对「{node_title}」的理解图，并标注关键概念。"),
            options: vec![],
            answer: vec![],
            explanation: "图片作答需要多模态模型评阅，当前只保存你的作答。".to_owned(),
            difficulty: 0.7,
        },
    ]
}

#[derive(Debug, Clone)]
pub struct PlannedQuestion {
    pub kind: crate::domain::QuizKind,
    pub prompt: String,
    pub options: Vec<String>,
    pub answer: Vec<String>,
    pub explanation: String,
    pub difficulty: f64,
}

/// Sequential lesson plan shown in the learning screen, mirroring the
/// prototype's "学习计划" checklist.
pub fn plan_steps(
    node_title: &str,
    concepts: &[String],
    minutes: i64,
) -> Vec<(String, String, crate::domain::LessonStepKind, String)> {
    let per_step = (minutes.max(8) / 4).max(2);
    let primary = concepts
        .first()
        .cloned()
        .unwrap_or_else(|| node_title.to_owned());

    vec![
        (
            "明确本课目标".to_owned(),
            format!("说清「{node_title}」要解决的问题"),
            crate::domain::LessonStepKind::Introduction,
            format!("{per_step} min"),
        ),
        (
            "理解核心概念".to_owned(),
            format!("围绕「{primary}」建立直觉"),
            crate::domain::LessonStepKind::Explanation,
            format!("{} min", per_step + 1),
        ),
        (
            "检查理解".to_owned(),
            "完成本节点的检查题".to_owned(),
            crate::domain::LessonStepKind::Quiz,
            format!("{per_step} min"),
        ),
        (
            "用自己的话复述".to_owned(),
            "把结论讲给一个不在场的人听".to_owned(),
            crate::domain::LessonStepKind::Reflection,
            format!("{per_step} min"),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_two_demo_topics() {
        assert_eq!(detect_topic_kind("射影几何"), TopicKind::Geometry);
        assert_eq!(detect_topic_kind("线性代数"), TopicKind::Algebra);
        assert_eq!(detect_topic_kind("和声学"), TopicKind::Custom);
    }

    #[test]
    fn default_route_has_six_ordered_nodes_with_a_quiz_and_a_review() {
        let plan = plan_route(&RouteBrief::new("射影几何"));
        assert_eq!(plan.nodes.len(), 6);
        assert_eq!(plan.nodes[0].title, "从欧氏到射影");
        assert_eq!(plan.nodes[2].kind, NodeKind::Review);
        assert_eq!(plan.nodes[4].kind, NodeKind::Quiz);
        for (index, node) in plan.nodes.iter().enumerate() {
            assert_eq!(node.key, format!("n{}", index + 1));
            assert_eq!(node.dependencies.len(), usize::from(index > 0));
        }
    }

    #[test]
    fn depth_and_experience_change_the_roadmap_length() {
        let mut brief = RouteBrief::new("线性代数");
        brief.desired_depth = Some("overview".into());
        let short = plan_route(&brief);
        assert_eq!(short.nodes.len(), 4);
        assert_eq!(short.nodes[2].kind, NodeKind::Review);

        let mut brief = RouteBrief::new("线性代数");
        brief.desired_depth = Some("deep".into());
        assert_eq!(plan_route(&brief).nodes.len(), 8);

        let mut brief = RouteBrief::new("线性代数");
        brief.experience_level = Some("advanced".into());
        let advanced = plan_route(&brief);
        assert_eq!(advanced.nodes.len(), 5);
        assert_ne!(advanced.nodes[0].title, "向量与空间");
    }

    #[test]
    fn every_node_gets_a_lesson_and_a_five_widget_quiz() {
        let plan = plan_route(&RouteBrief::new("群论"));
        for node in &plan.nodes {
            let lesson = plan_lesson(&node.title, &node.objective, &node.concepts, "群论");
            assert!(lesson.explanation.contains(&node.title));
            assert!(!lesson.key_points.is_empty());

            let quiz = plan_quiz(&node.title, &node.objective, &node.concepts);
            assert_eq!(quiz.len(), 5);
            let kinds: Vec<_> = quiz.iter().map(|question| question.kind).collect();
            assert_eq!(
                kinds,
                vec![
                    crate::domain::QuizKind::Single,
                    crate::domain::QuizKind::Multi,
                    crate::domain::QuizKind::Fill,
                    crate::domain::QuizKind::Short,
                    crate::domain::QuizKind::Image,
                ]
            );
            // Objective questions must be locally gradable; subjective ones must not.
            assert!(!quiz[0].answer.is_empty());
            assert!(quiz[3].answer.is_empty());
        }
    }

    #[test]
    fn steps_are_ordered_and_labelled() {
        let steps = plan_steps("无穷远点", &["方向".to_owned()], 17);
        assert_eq!(steps.len(), 4);
        assert_eq!(steps[0].2, crate::domain::LessonStepKind::Introduction);
        assert_eq!(steps[3].2, crate::domain::LessonStepKind::Reflection);
        assert!(steps.iter().all(|step| step.3.ends_with("min")));
    }
}
