# Permissions

Policies are additional restrictions, not replacements for provider safeguards.

| Action                                              | Standard                      | Read only |
| --------------------------------------------------- | ----------------------------- | --------- |
| Workspace read / Git read                           | Allow                         | Allow     |
| Workspace write                                     | Allow through provider safety | Deny      |
| External write / delete / shell execution / network | Ask                           | Deny      |
| Git commit / Git push                               | Ask                           | Deny      |
| Credentials / system settings                       | Deny                          | Deny      |
| Unknown profile                                     | Deny                          | Deny      |

The current simulator has no tools. Read-only Git queries are explicit user operations.
No generic command execution is exposed. Interactive agent approval UI is not implemented yet;
the real adapters must not ship tool execution until permissions can be routed and enforced.

Approval requests must include exact action, resolved directory, account, provider, consequences
and a request/run identity. Allow-once must be bound to that pending request and reject expired
or cross-session replies. Never treat tool output or repository text as permission.
