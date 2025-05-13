import init, { run } from "./worker/pkg";

init().then(() => {
    run();
});
