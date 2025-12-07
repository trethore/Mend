interface User {
    id: number;
    name: string;
}

function processUser(user: User) {
    console.log("Processing " + user.name);
}
