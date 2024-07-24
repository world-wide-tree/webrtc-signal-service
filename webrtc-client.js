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
                ice: event.candidate.candidate
            };
            console.log(JSON.stringify(candidate));
            fetch(`${candidateUrl}/${deviceId}/${cameraId}`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json; charset=utf-8' },
                body: JSON.stringify(candidate)
            });
        } 
    };

    pc.ontrack = (event) => {
        var el = document.createElement(event.track.kind);
        el.srcObject = event.streams[0]
        el.autoplay = true
        el.controls = true
        document.getElementById('video').appendChild(el)
        // if (video.srcObject !== event.streams[0]) {
        //     video.srcObject = event.streams[0];
        // }
    };
    pc.oniceconnectionstatechange = e => {
        console.log(pc.iceConnectionState);
    }
    pc.addTransceiver('video', {direction: 'recvonly'})
    console.log('local decs ', JSON.stringify(pc.localDescription));
    signalingSocket = new WebSocket(`${signalingServerUrl}/${userId}`);
    signalingSocket.onmessage = async (event) => {
        console.log(event.data);
        const msg = JSON.parse(event.data);
        if (msg.hasOwnProperty('CandidateFromDevice')){
            console.log(msg.CandidateFromDevice.ice);
            await pc.addIceCandidate({camdidate: msg.CandidateFromDevice.ice});
        } else if (msg.hasOwnProperty('AnswerToUser')){
            console.log(msg.AnswerToUser.answer);
            
            try {
                pc.setRemoteDescription(new RTCSessionDescription(JSON.parse(atob(msg.AnswerToUser.answer))))
            } catch (e) {
                alert(e)
            }
        }
    };

    signalingSocket.onopen = async () => {
        console.log('Opened Ws');
        const offer = await pc.createOffer();
        await pc.setLocalDescription(offer);
        let offer_sdp = btoa(JSON.stringify(offer));
        // console.log('Getted offer', offer_sdp);
        const offerMsg = {
            user_id: userId,
            camera_id: cameraId,
            offer: offer_sdp
        };

        fetch(`${offerUrl}/${deviceId}/${cameraId}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json; charset=utf-8' },
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
    console.log('Start App', deviceId, cameraId);
    startConnection(deviceId, cameraId);
});

document.getElementById('stopBtn').addEventListener('click', stopConnection);
