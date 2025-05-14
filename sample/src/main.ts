import "./style.css";

import init, { Client, Error } from "./main/pkg";

document.querySelector<HTMLDivElement>("#app")!.innerHTML = `
<div>
    <h1>Combadge Sample App</h1>

    <p class="read-the-docs" style="margin-bottom:2em;">
        View the source <a href="https://github.com/tavern-works/combadge/tree/main/sample">here</a>
    </p>

    <div class="line">
        Add some numbers
        <input type="number" id="sumInputA" value="3">
        <input type="number" id="sumInputB" value="5">
        <button id="sum" type="button">Compute Sum</button>
        <input id="sumOutput" readonly style="text-align:center;">
    </div>

    <div class="line">
        Try to parse a number
        <input id="parseInput" value="7">
        <button id="parse" type="button">Parse</button>
        <p id="parseOutput"></p>
    </div>

    <div class="line">
        Display a message using a callback
        <input id="message" style="width:20rem;" value="Hello, world!">
        <button id="display" type="button">Display</button>
    </div>

    <div class="line">
        Run an operation without blocking the main thread
        <button id="block" type="button">Run</button>
        <p id="blockOutput"></p>
    </div>

    <div class="line">
        See the effect of zero-copy transfers between threads*
        <button id="doublePostable" type="button">Copy</button>
        <button id="doubleTransferable" type="button">Don't copy</button>
        <p id="doubleOutput"></p>
    </div>

    <div class="line">
        Get a value from the future
        <button id="getFuture" type="button">Get future time</button>
        <p id="futureOutput" style="font-size: 70%"></p>
    </div>

    <p class="read-the-docs" style="font-size: 60%">* Improves performance on Chrome, but not Firefox</p>
</div>
`;

function buildArray(): Uint32Array {
    const array = new Uint32Array(100000000);
    for (let index = 0; index < array.length; ++index) {
        array[index] = index;
    }
    return array;
}

init().then(() => {
    const worker = new Worker(new URL("./worker.ts", import.meta.url), {
        type: "module",
    });

    const client = new Client(worker);

    document.getElementById("sum")!.onclick = () => {
        const a = (document.getElementById("sumInputA")! as HTMLInputElement)
            .valueAsNumber;
        const b = (document.getElementById("sumInputB")! as HTMLInputElement)
            .valueAsNumber;
        client.add(a, b).then((sum) => {
            (document.getElementById("sumOutput")! as HTMLInputElement).value =
                sum.toString();
        });
    };

    document.getElementById("parse")!.onclick = () => {
        const input = (
            document.getElementById("parseInput") as HTMLInputElement
        ).value;
        client
            .parse(input)
            .then((parsed) => {
                const output = document.getElementById(
                    "parseOutput",
                ) as HTMLParagraphElement;
                output.innerText = `Parsed '${input}' as ${parsed}`;
                output.classList.remove("error");
            })
            .catch((error) => {
                const output = document.getElementById(
                    "parseOutput",
                ) as HTMLParagraphElement;
                const errorString = (error as Error).toString();
                output.innerText = `Error: ${errorString}`;
                output.classList.add("error");
            });
    };

    document.getElementById("display")!.onclick = () => {
        const input = (document.getElementById("message") as HTMLInputElement)
            .value;
        client.callWithMessage(
            (message: string) => window.alert(message),
            input,
        );
    };

    document.getElementById("block")!.onclick = () => {
        const output = document.getElementById(
            "blockOutput",
        )! as HTMLParagraphElement;
        output.innerHTML = "Processing... <span class='loader'></span>";
        client.blockThread().then(() => (output.innerText = "Done!"));
    };

    document.getElementById("doublePostable")!.onclick = () => {
        const array = buildArray();
        const start = performance.now();
        client.doublePostable(array).then((doubled) => {
            if (doubled[100] !== 200) {
                console.error!("failed to double");
                return;
            }

            const duration = Math.round(performance.now() - start);
            const output = document.getElementById(
                "doubleOutput",
            )! as HTMLParagraphElement;
            output.innerText = `Passed ${doubled.length} elements in ${duration} ms`;
        });
    };

    document.getElementById("doubleTransferable")!.onclick = () => {
        const array = buildArray();
        const start = performance.now();
        client.doubleTransferable(array).then((doubled) => {
            if (doubled[100] !== 200) {
                console.error!("failed to double");
                return;
            }

            const duration = Math.round(performance.now() - start);
            const output = document.getElementById(
                "doubleOutput",
            )! as HTMLParagraphElement;
            output.innerText = `Passed ${doubled.length} elements in ${duration} ms`;
        });
    };

    document.getElementById("getFuture")!.onclick = () => {
        const output = document.getElementById(
            "futureOutput",
        )! as HTMLParagraphElement;
        output.innerHTML = "Ping " + new Date().toTimeString() + "<br>";
        client
            .getFuture()
            .then((futureTime) => (output!.innerText += "Pong " + futureTime));
    };
});
