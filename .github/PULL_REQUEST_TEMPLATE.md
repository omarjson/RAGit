name: 🔄 Pull Request
description: Submit a change to RAGit
title: "[PR] "
labels: [pr]
body:
  - type: textarea
    id: summary
    attributes:
      label: Summary
      description: What does this PR change and why?
    validations:
      required: true
  - type: input
    id: issue
    attributes:
      label: Related issue
      placeholder: "Closes #123"
  - type: checkboxes
    id: checks
    attributes:
      label: Checklist
      options:
        - label: "`cargo check` passes"
          required: true
        - label: "`npm run build` passes"
          required: true
        - label: "I updated docs/tests where needed"
          required: false
        - label: "I read CONTRIBUTING.md"
          required: true
  - type: textarea
    id: notes
    attributes:
      label: Test plan / manual verification
