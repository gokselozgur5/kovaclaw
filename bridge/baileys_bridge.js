import { makeWASocket, useMultiFileAuthState, fetchLatestBaileysVersion, makeCacheableSignalKeyStore, DisconnectReason } from '@whiskeysockets/baileys';
import { Boom } from '@hapi/boom';
import { createInterface } from 'readline';
import pino from 'pino';
import qrcode from 'qrcode-terminal';

const AUTH_DIR = process.env.BAILEYS_AUTH_DIR || './auth_state';
const rl = createInterface({ input: process.stdin });

function send(event) {
    process.stdout.write(JSON.stringify(event) + '\n');
}

async function start() {
    const logger = pino({ level: 'silent' });
    const { state, saveCreds } = await useMultiFileAuthState(AUTH_DIR);
    const { version } = await fetchLatestBaileysVersion();

    const sock = makeWASocket({
        auth: {
            creds: state.creds,
            keys: makeCacheableSignalKeyStore(state.keys, logger),
        },
        version,
        logger,
        printQRInTerminal: false,
        browser: ['KovaClaw', 'cli', '0.1.0'],
        syncFullHistory: false,
        markOnlineOnConnect: false,
    });

    sock.ev.on('creds.update', saveCreds);

    sock.ev.on('connection.update', ({ connection, lastDisconnect, qr }) => {
        if (qr) {
            process.stderr.write('Scan this QR in WhatsApp (Linked Devices):\n');
            qrcode.generate(qr, { small: true }, (code) => process.stderr.write(code + '\n'));
            send({ type: 'qr', data: qr });
        }
        if (connection === 'close') {
            const reason = new Boom(lastDisconnect?.error)?.output?.statusCode;
            if (reason === DisconnectReason.loggedOut) {
                send({ type: 'disconnected', reason: 'logged_out' });
                process.exit(1);
            }
            process.stderr.write(`Connection closed (reason: ${reason}), reconnecting in 3s...\n`);
            setTimeout(start, 3000);
            return; // Don't process further
        }
        if (connection === 'open') {
            send({ type: 'connected' });
        }
    });

    if (sock.ws && typeof sock.ws.on === 'function') {
        sock.ws.on('error', (err) => {
            process.stderr.write(`WS error: ${err.message}\n`);
        });
    }

    sock.ev.on('messages.upsert', ({ messages, type }) => {
        // Only process real-time notifications, not history sync
        if (type !== 'notify') return;
        for (const msg of messages) {
            const text = msg.message?.conversation
                || msg.message?.extendedTextMessage?.text
                || '';
            if (!text) continue;

            send({
                type: 'message',
                jid: msg.key.remoteJid,
                text,
                pushName: msg.pushName || '',
                messageId: msg.key.id,
                fromMe: msg.key.fromMe || false,
            });
        }
    });

    rl.on('line', async (line) => {
        try {
            const cmd = JSON.parse(line);
            if (cmd.type === 'send') {
                await sock.sendMessage(cmd.jid, { text: cmd.text });
                send({ type: 'sent', jid: cmd.jid, messageId: cmd.messageId || '' });
            }
        } catch (e) {
            send({ type: 'error', message: e.message });
        }
    });
}

start().catch((e) => {
    send({ type: 'error', message: e.message });
    process.exit(1);
});
