const fs = require('fs');

const file = 'android/feature/learning/src/main/java/com/schoolos/android/feature/learning/MaterialCreatorScreen.kt';
let c = fs.readFileSync(file, 'utf8');

const oldTarget = `        val targetClassId = unifiedClasses.findClass(selectedClass)?.primaryId
            ?: state.availableClasses.find { it.name == selectedClass }?.id
            ?: selectedClass
        val targetSubjectId = state.availableSubjects.find { it.name == selectedSubject }?.id
        val startP = startPageInput.toIntOrNull() ?: 1
        val endP = endPageInput.toIntOrNull() ?: minOf(startP + 10, selectedBook?.totalPages ?: 100)

        MaterialReviewAndPublishScreen(
            title = title.trim(),
            description = description.trim(),
            selectedType = selectedType,
            mediaUrl = mediaUrl.trim(),
            contentBody = contentBody.trim(),
            selectedClass = selectedClass,
            selectedSubject = selectedSubject,
            selectedBook = selectedBook,
            startPage = startP.toString(),
            endPage = endP.toString(),
            isLoading = state.isLoading,
            onBackToEdit = { showPrePublishDialog = false },
            onConfirmPublish = { isScheduled, releaseAtIso ->
                if (selectedType == MaterialType.DOCUMENT && pdfSourceMode == "SIBI" && selectedBook != null) {
                    viewModel.assignReadingBook(
                        book = selectedBook!!,
                        startPage = startP,
                        endPage = endP,
                        instructions = description.trim().ifBlank { null },
                        classId = targetClassId,
                        subjectId = targetSubjectId,
                        releaseAt = releaseAtIso,
                        visibility = if (isScheduled) "draft" else "published",
                    )
                } else {
                    val fullDesc = "$selectedSubject • $selectedClass • $description"
                    viewModel.createMaterial(
                        title = title.trim(),
                        description = fullDesc.trim(),
                        materialType = selectedType,
                        mediaUrl = mediaUrl.trim().ifBlank { null },
                        subject = selectedSubject,
                        classId = targetClassId,
                        subjectId = targetSubjectId,
                        visibility = if (isScheduled) "draft" else "published",
                        releaseAt = releaseAtIso,
                    )
                }
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
        val targetSubjectId = state.availableSubjects.find { it.name == selectedSubject }?.id
        val startP = startPageInput.toIntOrNull() ?: 1
        val endP = endPageInput.toIntOrNull() ?: minOf(startP + 10, selectedBook?.totalPages ?: 100)

        MaterialReviewAndPublishScreen(
            title = title.trim(),
            description = description.trim(),
            selectedType = selectedType,
            mediaUrl = mediaUrl.trim(),
            contentBody = contentBody.trim(),
            selectedClass = selectedClass,
            selectedSubject = selectedSubject,
            selectedBook = selectedBook,
            startPage = startP.toString(),
            endPage = endP.toString(),
            isLoading = state.isLoading,
            onBackToEdit = { showPrePublishDialog = false },
            onConfirmPublish = { isScheduled, releaseAtIso ->
                if (selectedType == MaterialType.DOCUMENT && pdfSourceMode == "SIBI" && selectedBook != null) {
                    viewModel.assignReadingBook(
                        book = selectedBook!!,
                        startPage = startP,
                        endPage = endP,
                        instructions = description.trim().ifBlank { null },
                        classId = targetClassId,
                        classIds = targetClassIds,
                        subjectId = targetSubjectId,
                        releaseAt = releaseAtIso,
                        visibility = if (isScheduled) "draft" else "published",
                    )
                } else {
                    val fullDesc = "$selectedSubject • $selectedClass • $description"
                    viewModel.createMaterial(
                        title = title.trim(),
                        description = fullDesc.trim(),
                        materialType = selectedType,
                        mediaUrl = mediaUrl.trim().ifBlank { null },
                        subject = selectedSubject,
                        classId = targetClassId,
                        classIds = targetClassIds,
                        subjectId = targetSubjectId,
                        visibility = if (isScheduled) "draft" else "published",
                        releaseAt = releaseAtIso,
                    )
                }
            }
        )`;

const oldSmart = `    LaunchedEffect(selectedClass, selectedSubject, state.availableBooks) {
        val resolvedName = unifiedClasses.findClass(selectedClass)?.memberClassNames?.firstOrNull() ?: selectedClass
        viewModel.updateRecommendation(resolvedName, selectedSubject)
    }`;

const newSmart = `    LaunchedEffect(selectedClass, subjectOptions) {
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
    }

    LaunchedEffect(selectedClass, selectedSubject, state.availableBooks) {
        val resolvedName = unifiedClasses.findClass(selectedClass)?.memberClassNames?.firstOrNull() ?: selectedClass
        viewModel.updateRecommendation(resolvedName, selectedSubject)
    }`;

c = c.replace(oldTarget.replace(/\n/g, '\r\n'), newTarget.replace(/\n/g, '\r\n'));
c = c.replace(oldSmart.replace(/\n/g, '\r\n'), newSmart.replace(/\n/g, '\r\n'));

fs.writeFileSync(file, c);
console.log('Done patching MaterialCreatorScreen.kt');

