const signalingServerUrl = 'ws://0.0.0.0:3030/user/signaling';
const offerUrl = 'http://0.0.0.0:3030/user/offer';
const candidateUrl = 'http://0.0.0.0:3030/user/candidate';
const userId = 'YourUserId'; // Replace with the actual user ID

let pc = null;
let signalingSocket = null;

async function startConnection(deviceId, cameraId) {
    pc = new RTCPeerConnection({
        iceServers: [{ urls: 'stun:stun.l.google.com:19302' }]
    });

    pc.onicecandidate = (event) => {
        if (event.candidate) {
            const candidate = {
                user_id: userId,
                ice: event.candidate
            };
            fetch(`${candidateUrl}/${deviceId}/${cameraId}`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(candidate)
            });
        }
    };

    pc.ontrack = (event) => {
        const video = document.getElementById('video');
        if (video.srcObject !== event.streams[0]) {
            video.srcObject = event.streams[0];
        }
    };

    signalingSocket = new WebSocket(`${signalingServerUrl}/${userId}`);
    signalingSocket.onmessage = async (event) => {
        const msg = JSON.parse(event.data);
        switch (msg.type) {
            case 'AnswerToUser':
                await pc.setRemoteDescription(new RTCSessionDescription(msg.answer));
                break;
            case 'CandidateFromDevice':
                await pc.addIceCandidate(new RTCIceCandidate(msg.ice));
                break;
            default:
                console.error('Unknown message type:', msg.type);
        }
    };

    signalingSocket.onopen = async () => {
        const offer = await pc.createOffer();
        await pc.setLocalDescription(offer);

        const offerMsg = {
            user_id: userId,
            camera_id: cameraId,
            offer: offer
        };

        fetch(`${offerUrl}/${deviceId}/${cameraId}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(offerMsg)
        });
    };
}

async function stopConnection() {
    if (pc) {
        pc.close();
        pc = null;
    }
    if (signalingSocket) {
        signalingSocket.close();
        signalingSocket = null;
    }
}

// HTML elements for user interface
document.getElementById('startBtn').addEventListener('click', () => {
    const deviceId = document.getElementById('deviceId').value;
    const cameraId = document.getElementById('cameraId').value;
    startConnection(deviceId, cameraId);
});

document.getElementById('stopBtn').addEventListener('click', stopConnection);
