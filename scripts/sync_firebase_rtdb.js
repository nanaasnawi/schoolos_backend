const https = require('https');

const base = 'https://akselerasi-edu-default-rtdb.asia-southeast1.firebasedatabase.app';
const threadId = '08ddca48-6427-4b51-8e5b-cb9225b6a033';

function put(path, data) {
  return new Promise((resolve, reject) => {
    const url = new URL(path, base);
    const body = JSON.stringify(data);
    const req = https.request(url, {
      method: 'PUT',
      headers: {
        'Content-Type': 'application/json',
        'Content-Length': Buffer.byteLength(body)
      }
    }, res => {
      let d = '';
      res.on('data', chunk => d += chunk);
      res.on('end', () => resolve(JSON.parse(d || '{}')));
    });
    req.on('error', reject);
    req.write(body);
    req.end();
  });
}

async function sync() {
  const metadata = {
    id: threadId,
    studentId: '01a096e3-d808-7650-918d-66c4d7aed825',
    studentName: 'RENALDI',
    studentClass: 'PAKET C 12B',
    teacherId: '01a092fc-af59-76b2-85f2-cca8b8d8d515',
    teacherName: 'Kepala Sekolah PKBM',
    subjectName: 'Konsultasi Kebijakan, Akademik & Pengaduan Warga Belajar',
    inquiryType: 'GENERAL',
    referenceTitle: 'Konsultasi Kebijakan, Akademik & Pengaduan Warga Belajar',
    referenceId: null,
    lastUpdated: 1794034054646,
    status: 'WAITING_REPLY'
  };

  const summary = {
    lastMessageContent: 'test',
    lastMessageAt: 1794034054646,
    lastSenderName: 'RENALDI',
    status: 'WAITING_REPLY',
    readStatusLabel: 'Menunggu Tanggapan Kepala Sekolah',
    teacherLastReadAt: 0,
    studentId: '01a096e3-d808-7650-918d-66c4d7aed825',
    studentName: 'RENALDI',
    teacherId: '01a092fc-af59-76b2-85f2-cca8b8d8d515',
    teacherName: 'Kepala Sekolah PKBM'
  };

  const message = {
    id: 'c1814382-3d5f-49de-8d30-1532bcb3f70e',
    threadId: threadId,
    senderId: '01a096e3-d808-7650-918d-66c4d7aed825',
    senderName: 'RENALDI',
    senderRole: 'STUDENT',
    content: 'test',
    timestamp: 1794034054646,
    isFromTeacher: false,
    isRead: false
  };

  console.log('Writing metadata to Firebase RTDB...');
  await put(`/inquiries/${threadId}/metadata.json`, metadata);
  console.log('Writing summary to Firebase RTDB...');
  await put(`/inquiries/${threadId}/summary.json`, summary);
  console.log('Writing message to Firebase RTDB...');
  await put(`/inquiries/${threadId}/messages/c1814382-3d5f-49de-8d30-1532bcb3f70e.json`, message);
  console.log('Done syncing to Firebase RTDB!');
}

sync().catch(console.error);
