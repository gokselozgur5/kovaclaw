const { default: makeWASocket, useMultiFileAuthState, DisconnectReason } = require('@whiskeysockets/baileys');
const { Boom } = require('@hapi/boom');
const readline = require('readline');

const AUTH_DIR = process.env.BAILEYS_AUTH_DIR || './auth_state';

const rl = readline.createInterface({ input: process.stdin });

function send(event) {
    process.stdout.write(JSON.stringify(event) + '\n');
}

async function start() {
    const { state, saveCreds } = await useMultiFileAuthState(AUTH_DIR);

    const sock = makeWASocket({
        auth: state,
        printQRInTerminal: true,
    });

    sock.ev.on('creds.update', saveCreds);

    sock.ev.on('connection.update', ({ connection, lastDisconnect }) => {
        if (connection === 'close') {
            const reason = new Boom(lastDisconnect?.error)?.output?.statusCode;
            if (reason === DisconnectReason.loggedOut) {
                send({ type: 'disconnected', reason: 'logged_out' });
                process.exit(1);
            }
            // reconnect
            setTimeout(start, 3000);
        } else if (connection === 'open') {
            send({ type: 'connected' });
        }
    });

    sock.ev.on('messages.upsert', ({ messages }) => {
        for (const msg of messages) {
            if (msg.key.fromMe) continue;
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
            });
        }
    });

    // Read outgoing messages from stdin (JSON lines)
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
