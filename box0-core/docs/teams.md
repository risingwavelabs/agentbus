# Teams

Multiple people can share a Box0 server. Each person gets their own API key and can be added to groups for shared access.

## Create a user

On the server machine (admin):

```bash
b0 invite alice
```

This prints Alice's API key.

## Create a shared group

```bash
b0 group create dev-team
```

```bash
b0 group add-member dev-team <alice-user-id>
```

## Connect from another machine

On Alice's laptop:

```bash
b0 login http://server:8080 --key <alice-key>
```

The CLI auto-configures the default group from Alice's membership.

## Work within a group

```bash
b0 worker add --group dev-team reviewer --instructions "Code reviewer."
```

```bash
b0 delegate --group dev-team reviewer "Review src/main.rs"
```

```bash
b0 wait
```

## How groups work

- Each user gets a personal group on creation.
- Users can be in multiple groups. Use `--group` to select which one.
- Workers in a group are visible to all group members.
- Workers outside a group are private to the creator.
- Only the worker creator can remove or update their workers.

## List groups

```bash
b0 group ls
```
