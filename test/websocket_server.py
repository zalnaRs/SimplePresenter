import asyncio
import websockets

# This is the handler function for a single WebSocket connection.
async def echo(websocket):
    """
    Asynchronously receives messages from a WebSocket and prints them to the console.
    """
    print("A client has connected.")
    try:
        # Loop indefinitely to handle incoming messages
        async for message in websocket:
            print(f"Received message from client: {message}")
            # You can also send a response back if you want, for example:
            # await websocket.send(f"Echoing back: {message}")

    except websockets.exceptions.ConnectionClosed as e:
        print(f"Connection closed by client: {e.code}, {e.reason}")
    finally:
        print("Client disconnected.")

# The main function to start the WebSocket server.
async def main():
    """
    Starts the WebSocket server on localhost at port 8765.
    """
    # Start the server and bind it to the echo handler.
    # The with-as statement ensures the server is properly shut down on exit.
    async with websockets.serve(echo, "localhost", 8765):
        print("WebSocket server started on ws://localhost:8765. Press Ctrl+C to stop.")
        # The server will run until interrupted.
        await asyncio.Future()  # Run forever

if __name__ == "__main__":
    # To run the server, you need to install the 'websockets' library first:
    # pip install websockets

    # Then you can run this script.
    # In a terminal, execute: python websocket_server.py
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\nServer stopped.")
