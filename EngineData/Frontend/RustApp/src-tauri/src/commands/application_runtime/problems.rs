use super::contract::{ApplicationProblem, ApplicationSubsystemSummaries};

fn problem(
    code: impl Into<String>,
    domain: &str,
    severity: &str,
    recoverable: bool,
    action: &str,
    message: impl Into<String>,
) -> ApplicationProblem {
    ApplicationProblem {
        code: code.into(),
        domain: domain.to_string(),
        severity: severity.to_string(),
        recoverable,
        action: action.to_string(),
        message: message.into(),
    }
}

pub fn collect_problems(summaries: &ApplicationSubsystemSummaries) -> Vec<ApplicationProblem> {
    let mut problems = Vec::new();
    let meeting = &summaries.meeting;

    if !meeting.blocker.is_empty() {
        problems.push(problem(
            meeting.blocker.clone(),
            "meeting",
            "blocking",
            true,
            "check_meeting",
            meeting.note.clone(),
        ));
    }

    for blocker in &meeting.preflight_blockers {
        let code = blocker.trim();
        if !code.is_empty() && !problems.iter().any(|item: &ApplicationProblem| item.code == code) {
            problems.push(problem(
                code,
                "meeting",
                "blocking",
                true,
                "fix_setup",
                "Meeting translation still needs attention before it can start.",
            ));
        }
    }

    if !summaries.worker.state.is_empty() && !summaries.worker.process_ready {
        problems.push(problem(
            format!("helper:{}", summaries.worker.state),
            "worker",
            if summaries.worker.state == "error" { "blocking" } else { "warning" },
            true,
            "fix_setup",
            summaries.worker.message.clone(),
        ));
    }

    if !summaries.audio.blocker.is_empty() {
        problems.push(problem(
            summaries.audio.blocker.clone(),
            "audio",
            "blocking",
            true,
            "select_microphone",
            summaries.audio.note.clone(),
        ));
    }

    problems
}
