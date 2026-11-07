const fs = require('fs');

const file = 'frontend/src/app/(dashboard)/dashboard/learning/assignments/create/page.tsx';
let c = fs.readFileSync(file, 'utf8');

// 1. In loadMasterData, make sure listClasses has all: true and fallback
const oldClassLoad = `listClasses({ query: { page_size: 100 } }).catch(() => null),`;
const newClassLoad = `listClasses({ query: { page_size: 100, all: true } as any }).catch(() => null),`;
if (c.includes(oldClassLoad)) {
  c = c.replace(oldClassLoad, newClassLoad);
  console.log('Replaced listClasses in assignments create');
}

// 2. Also enhance classRes parsing with fallback like materials/create
const oldClassResCheck = `        if (classRes?.data?.data) {
          const list = classRes.data.data;
          setClassesList(list);
          if (list.length > 0) {
            setClassName(list[0].name);
            setSelectedClassNames(prev => prev.length > 0 ? prev : [list[0].name]);
          }
        }`;

const newClassResCheck = `        let cList = Array.isArray(classRes?.data?.data)
          ? classRes.data.data
          : Array.isArray(classRes?.data)
            ? classRes.data
            : [];
        if (cList.length === 0) {
          try {
            const fallbackRes = await fetch(getApiUrl('/api/v1/academic/classes?all=true&page_size=100'), {
              headers: token ? { Authorization: \`Bearer \${token}\` } : {},
            });
            if (fallbackRes.ok) {
              const fbJson = await fallbackRes.json();
              cList = Array.isArray(fbJson?.data?.data) ? fbJson.data.data : (Array.isArray(fbJson?.data) ? fbJson.data : []);
            }
          } catch {}
        }
        if (cList.length > 0) {
          setClassesList(cList);
          setClassName(cList[0].name);
          setSelectedClassNames(prev => prev.length > 0 ? prev : [cList[0].name]);
        }`;

if (c.includes(oldClassResCheck)) {
  c = c.replace(oldClassResCheck, newClassResCheck);
  console.log('Enhanced classRes parsing with fallback in assignments create');
}

// 3. Make sure targetClassObj?.id is properly passed for each target class
const oldResponsesMap = `      const responses = await Promise.all(
        targetClasses.map(async (clsName) => {
          const targetClassObj = classesList.find(c => 
            c.name.toLowerCase().replace(/\\s+/g, '') === clsName.toLowerCase().replace(/\\s+/g, '') || c.id === clsName
          );

          const payload = {
            title: title.trim(),
            description: \`\${subjectName || 'Umum'} • \${targetClassObj?.name || clsName || 'Semua Rombel'} • \${effectiveTeacherName} • \${instructions.slice(0, 100) || 'Tugas Pembelajaran Terstruktur'}\`,
            instructions: instructions.trim() || undefined,
            max_score: computedMaxScore,
            due_at: formattedDueAt,
            release_at: scheduledReleaseAt,
            status: targetStatus,
            replace_previous: targetStatus === 'published' ? replacePrevious : false,
            assignment_type: assignmentFormat === 'HOMEWORK_PR' ? 'HOMEWORK' : 'QUIZ',
            class_id: targetClassObj?.id || clsName,
            subject_id: targetSubjectObj?.id || undefined,
            teacher_id: targetTeacherObj?.id || undefined,
            questions: payloadQuestions.length > 0 ? payloadQuestions : undefined,
          };

          return fetch(getApiUrl('/api/v1/learning/assignments'), {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              ...(token ? { Authorization: \`Bearer \${token}\` } : {})
            },
            body: JSON.stringify(payload)
          });
        })
      );`;

const newResponsesMap = `      const responses = await Promise.all(
        targetClasses.map(async (clsName) => {
          const targetClassObj = classesList.find(c => 
            c.name.toLowerCase().replace(/\\s+/g, '') === clsName.toLowerCase().replace(/\\s+/g, '') || c.id === clsName
          );
          const resolvedClassId = targetClassObj?.id || (clsName.length === 36 ? clsName : undefined);

          const payload = {
            title: title.trim(),
            description: \`\${subjectName || 'Umum'} • \${targetClassObj?.name || clsName || 'Semua Rombel'} • \${effectiveTeacherName} • \${instructions.slice(0, 100) || 'Tugas Pembelajaran Terstruktur'}\`,
            instructions: instructions.trim() || undefined,
            max_score: computedMaxScore,
            due_at: formattedDueAt,
            release_at: scheduledReleaseAt,
            status: targetStatus,
            replace_previous: targetStatus === 'published' ? replacePrevious : false,
            assignment_type: assignmentFormat === 'HOMEWORK_PR' ? 'HOMEWORK' : 'QUIZ',
            class_id: resolvedClassId,
            subject_id: targetSubjectObj?.id || undefined,
            teacher_id: targetTeacherObj?.id || undefined,
            questions: payloadQuestions.length > 0 ? payloadQuestions : undefined,
          };

          return fetch(getApiUrl('/api/v1/learning/assignments'), {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              ...(token ? { Authorization: \`Bearer \${token}\` } : {})
            },
            body: JSON.stringify(payload)
          });
        })
      );`;

if (c.includes(oldResponsesMap)) {
  c = c.replace(oldResponsesMap, newResponsesMap);
  console.log('Replaced responses map in assignments create');
}

fs.writeFileSync(file, c);
console.log('Successfully written to assignments create page.tsx');

