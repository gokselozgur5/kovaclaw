import { makeWASocket, useMultiFileAuthState } from '@whiskeysockets/baileys';

const { state, saveCreds } = await useMultiFileAuthState(process.env.BAILEYS_AUTH_DIR);
const sock = makeWASocket({ auth: state });

sock.ev.on('creds.update', saveCreds);

sock.ev.on('connection.update', (u) => {
  console.log('update:', JSON.stringify(u));
  if (u.connection === 'open') console.log('CONNECTED!');
});

sock.ev.on('messages.upsert', ({ messages }) => {
  for (const m of messages) {
    if (m.key.fromMe) continue;
    const text = m.message?.conversation || m.message?.extendedTextMessage?.text || '';
    if (text) console.log('MSG:', m.key.remoteJid, text);
  }
});
