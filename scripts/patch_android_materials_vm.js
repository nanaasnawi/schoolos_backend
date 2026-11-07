const fs = require('fs');

const file = 'android/feature/learning/src/main/java/com/schoolos/android/feature/learning/MaterialCreatorViewModel.kt';
let c = fs.readFileSync(file, 'utf8');

const oldCreate = `    fun createMaterial(
        title: String,
        description: String?,
        materialType: MaterialType,
        mediaUrl: String?,
        subject: String,
        classId: String?,
        subjectId: String? = null,
        visibility: String? = null,
        releaseAt: String? = null,
    ) {
        viewModelScope.launch {
            _state.value = _state.value.copy(isLoading = true, error = null)
            
            // Serialize content blocks if type is Article or Image(Infografis)
            val finalContentBody = if (materialType == MaterialType.ARTICLE || materialType == MaterialType.IMAGE || materialType == MaterialType.INFOGRAPHIC) {
                try {
                    Json.encodeToString(contentBlocks.value)
                } catch (_: Exception) {
                    null
                }
            } else {
                null
            }

            repository.createMaterial(
                title = title,
                description = description,
                materialType = materialType,
                contentBody = finalContentBody,
                mediaUrl = mediaUrl,
                subject = subject,
                classId = classId,
                subjectId = subjectId,
                visibility = visibility,
                releaseAt = releaseAt,
            ).onSuccess {
                _state.value = _state.value.copy(isLoading = false, success = true)
            }.onFailure { err ->
                _state.value = _state.value.copy(
                    isLoading = false,
                    error = err.toHumanReadableError("Gagal membuat materi ajar. Periksa koneksi internet Anda.")
                )
            }
        }
    }`;

const newCreate = `    fun createMaterial(
        title: String,
        description: String?,
        materialType: MaterialType,
        mediaUrl: String?,
        subject: String,
        classId: String? = null,
        classIds: List<String> = emptyList(),
        subjectId: String? = null,
        visibility: String? = null,
        releaseAt: String? = null,
    ) {
        viewModelScope.launch {
            _state.value = _state.value.copy(isLoading = true, error = null)
            
            // Serialize content blocks if type is Article or Image(Infografis)
            val finalContentBody = if (materialType == MaterialType.ARTICLE || materialType == MaterialType.IMAGE || materialType == MaterialType.INFOGRAPHIC) {
                try {
                    Json.encodeToString(contentBlocks.value)
                } catch (_: Exception) {
                    null
                }
            } else {
                null
            }

            val targetList = if (classIds.isNotEmpty()) classIds else listOfNotNull(classId?.ifBlank { null })
            val effectiveClasses = if (targetList.isNotEmpty()) targetList else listOf("")

            var successCount = 0
            var lastError: Throwable? = null

            for (cid in effectiveClasses) {
                val res = repository.createMaterial(
                    title = title,
                    description = description,
                    materialType = materialType,
                    contentBody = finalContentBody,
                    mediaUrl = mediaUrl,
                    subject = subject,
                    classId = cid.ifBlank { null },
                    subjectId = subjectId,
                    visibility = visibility,
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
                    error = lastError?.toHumanReadableError("Gagal membuat materi ajar. Periksa koneksi internet Anda.")
                )
            }
        }
    }`;

const oldAssign = `    fun assignReadingBook(
        book: com.schoolos.android.domain.model.LibraryBook,
        startPage: Int,
        endPage: Int,
        instructions: String?,
        classId: String,
        subjectId: String?,
        releaseAt: String? = null,
        visibility: String? = null,
    ) {
        viewModelScope.launch {
            _state.value = _state.value.copy(isLoading = true, error = null)
            val title = "Materi Bacaan: \${book.title} (Hal. $startPage–$endPage)"
            repository.assignReadingMaterial(
                bookId = book.id,
                title = title,
                instructions = instructions,
                classId = classId,
                subjectId = subjectId,
                startPage = startPage,
                endPage = endPage,
                releaseAt = releaseAt,
                visibility = visibility,
            ).onSuccess {
                _state.value = _state.value.copy(isLoading = false, success = true)
            }.onFailure { err ->
                _state.value = _state.value.copy(
                    isLoading = false,
                    error = err.toHumanReadableError("Gagal menugaskan materi buku perpustakaan.")
                )
            }
        }
    }`;

const newAssign = `    fun assignReadingBook(
        book: com.schoolos.android.domain.model.LibraryBook,
        startPage: Int,
        endPage: Int,
        instructions: String?,
        classId: String = "",
        classIds: List<String> = emptyList(),
        subjectId: String?,
        releaseAt: String? = null,
        visibility: String? = null,
    ) {
        viewModelScope.launch {
            _state.value = _state.value.copy(isLoading = true, error = null)
            val title = "Materi Bacaan: \${book.title} (Hal. $startPage–$endPage)"
            val targetList = if (classIds.isNotEmpty()) classIds else listOfNotNull(classId.ifBlank { null })
            val effectiveClasses = if (targetList.isNotEmpty()) targetList else listOf(classId)

            var successCount = 0
            var lastError: Throwable? = null

            for (cid in effectiveClasses) {
                val res = repository.assignReadingMaterial(
                    bookId = book.id,
                    title = title,
                    instructions = instructions,
                    classId = cid,
                    subjectId = subjectId,
                    startPage = startPage,
                    endPage = endPage,
                    releaseAt = releaseAt,
                    visibility = visibility,
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
                    error = lastError?.toHumanReadableError("Gagal menugaskan materi buku perpustakaan.")
                )
            }
        }
    }`;

c = c.replace(oldCreate.replace(/\n/g, '\r\n'), newCreate.replace(/\n/g, '\r\n'));
c = c.replace(oldAssign.replace(/\n/g, '\r\n'), newAssign.replace(/\n/g, '\r\n'));

fs.writeFileSync(file, c);
console.log('Done patching MaterialCreatorViewModel.kt');

