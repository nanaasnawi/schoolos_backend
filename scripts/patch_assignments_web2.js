const fs = require('fs');

const file = 'frontend/src/app/(dashboard)/dashboard/learning/assignments/create/page.tsx';
let c = fs.readFileSync(file, 'utf8');

c = c.replace(
  `        if (classRes?.data?.data) {
          const list = classRes.data.data;
          setClassesList(list);
          if (list.length > 0) {
            setClassName(list[0].name);
            setSelectedClassNames(prev => prev.length > 0 ? prev : [list[0].name]);
          }
        }`.replace(/\n/g, '\r\n'),
  `        let cList = Array.isArray(classRes?.data?.data)
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
        }`.replace(/\n/g, '\r\n')
);

c = c.replace(
  `          return fetch(getApiUrl('/api/v1/learning/assignments'), {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              ...(token ? { Authorization: \`Bearer \${token}\` } : {})
            },
            body: JSON.stringify(payload)
          });`.replace(/\n/g, '\r\n'),
  `          return fetch(getApiUrl('/api/v1/learning/assignments'), {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              ...(token ? { Authorization: \`Bearer \${token}\` } : {})
            },
            body: JSON.stringify({
              ...payload,
              class_id: targetClassObj?.id || clsName
            })
          });`.replace(/\n/g, '\r\n')
);

fs.writeFileSync(file, c);
console.log('Done patching assignments create');

