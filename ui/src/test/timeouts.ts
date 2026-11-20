import { configure } from "@testing-library/react";

// The `findBy` queries and `waitFor` wait up to five seconds instead of one,
// so a machine busy with other work does not fail a test whose interface is
// only slow to arrive.
configure({ asyncUtilTimeout: 5_000 });
