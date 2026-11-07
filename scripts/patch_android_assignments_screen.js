const fs = require('fs');

const file = 'android/feature/assignments/src/main/java/com/schoolos/android/feature/assignments/AssignmentCreatorScreen.kt';
let c = fs.readFileSync(file, 'utf8');

const oldTarget = `        val targetClassId = unifiedClasses.findClass(selectedClass)?.primaryId
            ?: state.availableClasses.find { it.name == selectedClass }?.id
            ?: selectedClass
        val parsedScore = customMaxScore.toIntOrNull() ?: 100
        val fullDesc = "$selectedSubject • $selectedClass • $description"

        AssignmentReviewAndPublishScreen(
            title = title.trim(),
            description = fullDesc.trim(),
            instructions = instructions.ifBlank { "Kerjakan tugas secara teliti dan kumpulkan sebelum batas waktu berakhir." },
            selectedClass = selectedClass,
            selectedSubject = selectedSubject,
            dueDays = selectedDueDays,
            maxScore = displayMaxScore,
            assignmentFormat = state.assignmentFormat,
            questions = state.questions,
            isLoading = state.isLoading,
            onBackToEdit = { showPrePublishReview = false },
            onConfirmPublish = { isScheduled, releaseAtIso ->
                viewModel.createAssignment(
                    title = title.trim(),
                    description = fullDesc.trim(),
                    instructions = instructions.ifBlank { "Kerjakan tugas secara teliti dan kumpulkan sebelum batas waktu berakhir." },
                    customMaxScore = parsedScore,
                    dueAt = calculatedDueDate,
                    classId = targetClassId,
                    status = if (isScheduled) "draft" else "published",
                    releaseAt = releaseAtIso,
                )
            }
        )`;

const newTarget = `        val matchedUnified = unifiedClasses.findClass(selectedClass)
        val targetClassIds = if (matchedUnified != null && matchedUnified.memberClassIds.isNotEmpty()) {
            matchedUnified.memberClassIds
        } else {
            val singleId = state.availableClasses.find { it.name == selectedClass }?.id ?: selectedClass
            listOf(singleId)
        }
        val targetClassId = targetClassIds.firstOrNull() ?: selectedClass
        val parsedScore = customMaxScore.toIntOrNull() ?: 100
        val fullDesc = "$selectedSubject • $selectedClass • $description"

        AssignmentReviewAndPublishScreen(
            title = title.trim(),
            description = fullDesc.trim(),
            instructions = instructions.ifBlank { "Kerjakan tugas secara teliti dan kumpulkan sebelum batas waktu berakhir." },
            selectedClass = selectedClass,
            selectedSubject = selectedSubject,
            dueDays = selectedDueDays,
            maxScore = displayMaxScore,
            assignmentFormat = state.assignmentFormat,
            questions = state.questions,
            isLoading = state.isLoading,
            onBackToEdit = { showPrePublishReview = false },
            onConfirmPublish = { isScheduled, releaseAtIso ->
                viewModel.createAssignment(
                    title = title.trim(),
                    description = fullDesc.trim(),
                    instructions = instructions.ifBlank { "Kerjakan tugas secara teliti dan kumpulkan sebelum batas waktu berakhir." },
                    customMaxScore = parsedScore,
                    dueAt = calculatedDueDate,
                    classId = targetClassId,
                    classIds = targetClassIds,
                    status = if (isScheduled) "draft" else "published",
                    releaseAt = releaseAtIso,
                )
            }
        )`;

const oldSmart = `    LaunchedEffect(subjectOptions, state.isSubjectLocked, state.teacherSubject) {
        if (state.isSubjectLocked && !state.teacherSubject.isNullOrBlank()) {
            val matched = subjectOptions.firstOrNull { 
                it.equals(state.teacherSubject!!, ignoreCase = true) ||
                it.contains(state.teacherSubject!!, ignoreCase = true) || 
                state.teacherSubject!!.contains(it, ignoreCase = true) 
            }
            selectedSubject = matched ?: subjectOptions.firstOrNull() ?: state.teacherSubject!!
        } else if (selectedSubject.isEmpty() && subjectOptions.isNotEmpty()) {
            selectedSubject = subjectOptions.first()
        }
    }`;

const newSmart = `    LaunchedEffect(subjectOptions, state.isSubjectLocked, state.teacherSubject) {
        if (state.isSubjectLocked && !state.teacherSubject.isNullOrBlank()) {
            val matched = subjectOptions.firstOrNull { 
                it.equals(state.teacherSubject!!, ignoreCase = true) ||
                it.contains(state.teacherSubject!!, ignoreCase = true) || 
                state.teacherSubject!!.contains(it, ignoreCase = true) 
            }
            selectedSubject = matched ?: subjectOptions.firstOrNull() ?: state.teacherSubject!!
        } else if (selectedSubject.isEmpty() && subjectOptions.isNotEmpty()) {
            selectedSubject = subjectOptions.first()
        }
    }

    LaunchedEffect(selectedClass, subjectOptions) {
        if (subjectOptions.size > 1 && selectedClass.isNotBlank()) {
            val upper = selectedClass.uppercase()
            val autoSubject = if (upper.contains("PAKET C") || upper.contains(" 10") || upper.contains(" 11") || upper.contains(" 12")) {
                subjectOptions.firstOrNull { it.contains("ekonomi", ignoreCase = true) || it.contains("geografi", ignoreCase = true) || it.contains("sosiologi", ignoreCase = true) }
            } else if (upper.contains("PAKET B") || upper.contains(" 7") || upper.contains(" 8") || upper.contains(" 9")) {
                subjectOptions.firstOrNull { it.contains("cirebon", ignoreCase = true) || it.contains("ipa", ignoreCase = true) || it.contains("ips", ignoreCase = true) }
            } else null
            if (autoSubject != null && autoSubject != selectedSubject) {
                selectedSubject = autoSubject
            }
        }
    }`;

c = c.replace(oldTarget.replace(/\n/g, '\r\n'), newTarget.replace(/\n/g, '\r\n'));
c = c.replace(oldSmart.replace(/\n/g, '\r\n'), newSmart.replace(/\n/g, '\r\n'));

fs.writeFileSync(file, c);
console.log('Done patching AssignmentCreatorScreen.kt');

