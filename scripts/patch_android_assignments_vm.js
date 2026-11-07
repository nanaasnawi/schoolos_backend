const fs = require('fs');

const file = 'android/feature/assignments/src/main/java/com/schoolos/android/feature/assignments/AssignmentCreatorViewModel.kt';
let c = fs.readFileSync(file, 'utf8');

// Also make sure UUID import exists
if (!c.includes('import java.util.UUID')) {
    c = c.replace('import androidx.lifecycle.ViewModel', 'import java.util.UUID\nimport androidx.lifecycle.ViewModel');
}

const oldCreate = `    fun createAssignment(
        title: String,
        description: String,
        instructions: String,
        customMaxScore: Int,
        dueAt: String,
        classId: String?,
        status: String? = null,
        releaseAt: String? = null,
    ) {
        viewModelScope.launch {
            _state.value = _state.value.copy(isLoading = true, error = null)

            val format = _state.value.assignmentFormat
            val isStructured = format == "STRUCTURED_QUESTIONS" || format == "HYBRID"

            // Filter valid questions: must have non-blank questionText
            val validQuestions = if (isStructured) {
                _state.value.questions
                    .filter { it.questionText.isNotBlank() }
                    .mapIndexed { idx, q ->
                        val validChoices = if (q.questionType == "MULTIPLE_CHOICE") {
                            q.choices.filter { it.choiceText.isNotBlank() }.mapIndexed { cIdx, c ->
                                c.copy(orderIndex = cIdx + 1)
                            }
                        } else emptyList()
                        q.copy(orderIndex = idx + 1, choices = validChoices)
                    }
            } else emptyList()

            val totalQuestionsPoints = validQuestions.sumOf { it.points ?: 10 }
            val computedMaxScore = if (isStructured && totalQuestionsPoints > 0) {
                totalQuestionsPoints
            } else {
                customMaxScore
            }

            val assignmentType = when (format) {
                "HOMEWORK_PR" -> "HOMEWORK"
                "HYBRID" -> "HYBRID"
                else -> "QUIZ"
            }

            repository.createAssignment(
                title = title,
                description = description,
                instructions = instructions,
                maxScore = computedMaxScore,
                dueAt = dueAt,
                classId = classId ?: "",
                assignmentType = assignmentType,
                questions = validQuestions,
                status = status,
                releaseAt = releaseAt,
            ).onSuccess {
                _state.value = _state.value.copy(isLoading = false, success = true)
            }.onFailure { e ->
                _state.value = _state.value.copy(
                    isLoading = false,
                    error = e.toHumanReadableError("Gagal membuat penugasan baru. Periksa koneksi internet Anda.")
                )
            }
        }
    }`;

const newCreate = `    fun createAssignment(
        title: String,
        description: String,
        instructions: String,
        customMaxScore: Int,
        dueAt: String,
        classId: String? = null,
        classIds: List<String> = emptyList(),
        status: String? = null,
        releaseAt: String? = null,
    ) {
        viewModelScope.launch {
            _state.value = _state.value.copy(isLoading = true, error = null)

            val format = _state.value.assignmentFormat
            val isStructured = format == "STRUCTURED_QUESTIONS" || format == "HYBRID"

            // Filter valid questions: must have non-blank questionText
            val validQuestions = if (isStructured) {
                _state.value.questions
                    .filter { it.questionText.isNotBlank() }
                    .mapIndexed { idx, q ->
                        val validChoices = if (q.questionType == "MULTIPLE_CHOICE") {
                            q.choices.filter { it.choiceText.isNotBlank() }.mapIndexed { cIdx, c ->
                                c.copy(orderIndex = cIdx + 1)
                            }
                        } else emptyList()
                        q.copy(orderIndex = idx + 1, choices = validChoices)
                    }
            } else emptyList()

            val totalQuestionsPoints = validQuestions.sumOf { it.points ?: 10 }
            val computedMaxScore = if (isStructured && totalQuestionsPoints > 0) {
                totalQuestionsPoints
            } else {
                customMaxScore
            }

            val assignmentType = when (format) {
                "HOMEWORK_PR" -> "HOMEWORK"
                "HYBRID" -> "HYBRID"
                else -> "QUIZ"
            }

            val targetList = if (classIds.isNotEmpty()) classIds else listOfNotNull(classId?.ifBlank { null })
            val effectiveClasses = if (targetList.isNotEmpty()) targetList else listOf("")

            var successCount = 0
            var lastError: Throwable? = null

            for (cid in effectiveClasses) {
                // Generate fresh UUIDs for questions and choices per class to avoid DB PK collisions
                val classQuestions = validQuestions.map { q ->
                    q.copy(
                        id = UUID.randomUUID().toString(),
                        choices = q.choices.map { c ->
                            c.copy(id = UUID.randomUUID().toString())
                        }
                    )
                }

                val res = repository.createAssignment(
                    title = title,
                    description = description,
                    instructions = instructions,
                    maxScore = computedMaxScore,
                    dueAt = dueAt,
                    classId = cid,
                    assignmentType = assignmentType,
                    questions = classQuestions,
                    status = status,
                    releaseAt = releaseAt,
                )
                if (res.isSuccess) {
                    successCount++
                } else {
                    lastError = res.exceptionOrNull()
                }
            }

            if (successCount > 0) {
                _state.value = _state.value.copy(isLoading = false, success = true)
            } else {
                _state.value = _state.value.copy(
                    isLoading = false,
                    error = lastError?.toHumanReadableError("Gagal membuat penugasan baru. Periksa koneksi internet Anda.")
                )
            }
        }
    }`;

c = c.replace(oldCreate.replace(/\n/g, '\r\n'), newCreate.replace(/\n/g, '\r\n'));

fs.writeFileSync(file, c);
console.log('Done patching AssignmentCreatorViewModel.kt');

