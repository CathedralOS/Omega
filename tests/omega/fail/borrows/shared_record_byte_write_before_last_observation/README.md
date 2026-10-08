# A future byte observation keeps its shared loan live

Writing the owner's valid UTF-8 field while a shared record reference still
needs to observe that field must reject. Exit release scheduling cannot retire
the descriptor before its last authored observation to permit the write.
