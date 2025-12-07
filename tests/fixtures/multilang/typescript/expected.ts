interface User {
    id: number;
    name: string;
    email?: string;
}

function processUser( user: User ) {
    console.log(`Processing ${user.name}`);
}